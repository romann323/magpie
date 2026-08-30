import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { listen } from '@tauri-apps/api/event'
import {
  getImage,
  getMagnifierContext,
  queryImages,
  setMagnifierCurrent,
  toAssetUrl,
} from '../ipc'
import type { MagnifierContext } from '../types'

/**
 * Root component of the standalone Magnifier window. Loaded by
 * `main.tsx` when `location.hash === '#magnifier'`.
 *
 * On mount it pulls a `MagnifierContext` from the Rust process (the
 * main window stashed it there right before spawning us). We then:
 *
 * 1. Fetch the current image directly via `get_image(id)` and display
 *    it — this is independent of any list query, so a bad filter or
 *    a slow `query_images` never blocks the picture from showing up.
 * 2. Fetch the filtered/sorted list in the background so ← / → can
 *    navigate the same set the main grid is showing. If that fails
 *    or is empty the arrows are simply disabled.
 */
export function MagnifierWindow() {
  const [ctx, setCtx] = useState<MagnifierContext | null>(null)
  const [currentId, setCurrentIdState] = useState<number | null>(null)
  const [contextError, setContextError] = useState<string | null>(null)

  useEffect(() => {
    let cancelled = false
    getMagnifierContext()
      .then((c) => {
        if (cancelled) return
        setCtx(c)
        setCurrentIdState(c.imageId)
      })
      .catch((e) => {
        if (cancelled) return
        setContextError(String((e as Error)?.message ?? e))
        setCtx({
          imageId: null,
          filter: {},
          sort: { by: 'takenAt', dir: 'desc' },
        })
      })
    return () => {
      cancelled = true
    }
  }, [])

  // The main window fires `app://magnifier-reset` when the user
  // re-triggers the magnifier while the popup is already open; we
  // refresh our context so we jump to the new image.
  useEffect(() => {
    let un: (() => void) | null = null
    listen<number>('app://magnifier-reset', async (e) => {
      const next = await getMagnifierContext().catch(() => null)
      if (next) {
        setCtx(next)
        setCurrentIdState(e.payload ?? next.imageId)
      }
    })
      .then((fn) => {
        un = fn
      })
      .catch(() => {})
    return () => {
      if (un) un()
    }
  }, [])

  // ---- Current image (direct fetch, independent of the list) ------
  const imageQ = useQuery({
    queryKey: ['magnifier-image', currentId],
    queryFn: () => getImage(currentId!),
    enabled: currentId != null,
  })

  // ---- Filtered/sorted list for prev/next navigation --------------
  const listQ = useQuery({
    queryKey: ['magnifier-list', ctx?.filter, ctx?.sort],
    queryFn: () =>
      queryImages({
        filter: ctx?.filter ?? {},
        sort: ctx?.sort ?? { by: 'takenAt', dir: 'desc' },
        page: { offset: 0, limit: 5000 },
      }),
    enabled: !!ctx,
  })

  const items = listQ.data?.items ?? []
  const idx = useMemo(
    () => (currentId == null ? -1 : items.findIndex((i) => i.id === currentId)),
    [items, currentId],
  )
  const cur = imageQ.data ?? null

  const setCurrentId = useCallback((id: number | null) => {
    setCurrentIdState(id)
    void setMagnifierCurrent(id).catch(() => {})
  }, [])

  const goPrev = useCallback(() => {
    if (idx > 0) setCurrentId(items[idx - 1]!.id)
  }, [idx, items, setCurrentId])

  const goNext = useCallback(() => {
    if (idx >= 0 && idx < items.length - 1) setCurrentId(items[idx + 1]!.id)
  }, [idx, items, setCurrentId])

  const closeMe = useCallback(() => {
    void getCurrentWindow().close()
  }, [])

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.preventDefault()
        closeMe()
      } else if (e.key === 'ArrowLeft' || e.key === 'ArrowUp') {
        e.preventDefault()
        goPrev()
      } else if (e.key === 'ArrowRight' || e.key === 'ArrowDown') {
        e.preventDefault()
        goNext()
      }
    }
    window.addEventListener('keydown', handler)
    return () => window.removeEventListener('keydown', handler)
  }, [closeMe, goPrev, goNext])

  useEffect(() => {
    const title = cur?.filename ? `${cur.filename} — Magnifier` : 'Magnifier'
    void getCurrentWindow().setTitle(title).catch(() => {})
  }, [cur?.filename])

  const imgRef = useRef<HTMLImageElement | null>(null)
  const [imgFailed, setImgFailed] = useState(false)

  useEffect(() => {
    setImgFailed(false)
  }, [cur?.path])

  return (
    <div className="h-full w-full grid grid-rows-[1fr_auto] bg-black text-slate-200 select-none">
      <div className="min-h-0 grid place-items-center overflow-hidden p-2">
        {cur && !imgFailed ? (
          <img
            ref={imgRef}
            src={toAssetUrl(cur.path)}
            alt={cur.filename}
            className="max-w-full max-h-full object-contain"
            draggable={false}
            onError={() => setImgFailed(true)}
          />
        ) : imgFailed && cur ? (
          <div className="text-slate-400 text-sm text-center px-8 max-w-lg">
            Could not display <span className="text-slate-200">{cur.filename}</span>.
            <br />
            <span className="text-slate-500 text-xs">
              The webview refused to load {cur.path}. This usually means the
              file was moved or deleted, or the file type isn&apos;t a picture
              format the OS can render (RAW, HEIC, video, PDF, …).
            </span>
          </div>
        ) : imageQ.isError ? (
          <div className="text-red-300 text-sm text-center px-8 max-w-lg">
            Could not load image: {(imageQ.error as Error)?.message ?? 'unknown error'}
          </div>
        ) : contextError ? (
          <div className="text-red-300 text-sm text-center px-8 max-w-lg">
            Could not read magnifier context: {contextError}
          </div>
        ) : currentId == null && ctx ? (
          <div className="text-slate-400 text-sm text-center px-8 max-w-lg">
            No image to display. Select a file in the main window and choose
            View → Magnifier (or double-click it).
          </div>
        ) : (
          <div className="text-slate-400 text-sm">Loading image…</div>
        )}
      </div>

      <div className="shrink-0 flex items-center justify-between gap-3 px-4 py-2 bg-black/80 text-xs">
        <div className="truncate max-w-[50%]" title={cur?.path ?? ''}>
          {cur?.title || cur?.filename || ''}
        </div>
        <div className="flex items-center gap-3">
          {idx >= 0 && items.length > 0 && (
            <span className="text-slate-400">
              {idx + 1} / {items.length}
            </span>
          )}
          <button
            type="button"
            className="btn"
            onClick={goPrev}
            disabled={idx <= 0}
            title="Previous (←)"
          >
            ‹ Prev
          </button>
          <button
            type="button"
            className="btn"
            onClick={goNext}
            disabled={idx < 0 || idx >= items.length - 1}
            title="Next (→)"
          >
            Next ›
          </button>
          <button
            type="button"
            className="btn"
            onClick={closeMe}
            title="Close (Esc)"
          >
            Close
          </button>
        </div>
      </div>
    </div>
  )
}
