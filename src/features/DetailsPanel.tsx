import { useEffect, useMemo, useRef, useState } from 'react'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { confirm } from '@tauri-apps/plugin-dialog'
import {
  batchUpdateMetadata,
  deleteImages,
  getImage,
  logFrontend,
  toAssetUrl,
  updateImageMetadata,
} from '../ipc'
import { useStore } from '../store'
import type { ImageDetails, MetadataPatch } from '../types'
import { StarRating } from './StarRating'
import { TagInput } from './TagInput'

export function DetailsPanel() {
  const selection = useStore((s) => s.selection)
  const setDetailsOpen = useStore((s) => s.setDetailsOpen)
  const ids = useMemo(() => [...selection.ids], [selection.ids])

  if (ids.length === 0) {
    return (
      <aside className="border-l border-surface-border p-4 flex flex-col h-full text-slate-500 text-sm">
        <TitleBar onClose={() => setDetailsOpen(false)} label="Details" />
        <div className="grid place-items-center flex-1 text-center px-4">
          Select an image to edit its metadata.
        </div>
      </aside>
    )
  }

  if (ids.length === 1) {
    return <SingleDetails id={ids[0]!} onClose={() => setDetailsOpen(false)} />
  }
  return <MultiDetails ids={ids} onClose={() => setDetailsOpen(false)} />
}

function TitleBar({ label, onClose }: { label: string; onClose: () => void }) {
  return (
    <div className="flex items-center justify-between mb-3">
      <div className="text-sm font-medium">{label}</div>
      <button className="btn-icon" onClick={onClose} title="Hide details">×</button>
    </div>
  )
}

function useDebouncedCallback<T extends (...args: never[]) => void>(
  fn: T,
  delay: number,
): (...args: Parameters<T>) => void {
  const fnRef = useRef(fn)
  fnRef.current = fn
  const timer = useRef<number | null>(null)
  useEffect(() => {
    return () => {
      if (timer.current !== null) window.clearTimeout(timer.current)
    }
  }, [])
  return (...args: Parameters<T>) => {
    if (timer.current !== null) window.clearTimeout(timer.current)
    timer.current = window.setTimeout(() => {
      fnRef.current(...args)
    }, delay)
  }
}

function SingleDetails({ id, onClose }: { id: number; onClose: () => void }) {
  const qc = useQueryClient()
  const clearSelection = useStore((s) => s.clearSelection)

  const q = useQuery({
    queryKey: ['image', id],
    queryFn: () => getImage(id),
  })

  // Local edit state. Only reset when the image *id* changes (or on first load).
  // This prevents refetches triggered by other mutations from stomping the user's
  // in-progress typing in the title/comment inputs.
  const [title, setTitle] = useState('')
  const [comment, setComment] = useState('')
  const [rating, setRating] = useState<number | null>(null)
  const [tags, setTags] = useState<string[]>([])
  const [saving, setSaving] = useState<string | null>(null)

  const lastLoadedId = useRef<number | null>(null)

  useEffect(() => {
    if (!q.data) return
    if (lastLoadedId.current === q.data.id) return
    lastLoadedId.current = q.data.id
    setTitle(q.data.title ?? '')
    setComment(q.data.comment ?? '')
    setRating(q.data.rating)
    setTags(q.data.tags)
  }, [q.data])

  const applyPatch = async (patch: MetadataPatch, label: string) => {
    setSaving(label)
    try {
      const updated = await updateImageMetadata(id, patch)
      // Update the cache directly so we don't trigger a refetch that would
      // stomp any in-progress typing in other fields.
      qc.setQueryData(['image', id], updated)
      // Grid & tag-cloud can refetch freely; they don't touch local edit state.
      qc.invalidateQueries({ queryKey: ['images'] })
      qc.invalidateQueries({ queryKey: ['tags'] })
    } catch (e) {
      console.error('metadata save failed', e)
    } finally {
      setSaving(null)
    }
  }

  // Debounced auto-save for text fields. onBlur also forces a save immediately.
  const debouncedSaveTitle = useDebouncedCallback((v: string) => {
    void applyPatch({ title: v.trim() === '' ? null : v }, 'title')
  }, 600)
  const debouncedSaveComment = useDebouncedCallback((v: string) => {
    void applyPatch({ comment: v.trim() === '' ? null : v }, 'comment')
  }, 600)

  const deleteMutation = useMutation({
    mutationFn: async () => {
      const yes = await confirm(
        `Move "${q.data?.filename ?? 'this image'}" to the Recycle Bin?\n\nYou can restore it from the Windows Recycle Bin afterwards.`,
        { title: 'Delete image', kind: 'warning', okLabel: 'Move to Recycle Bin', cancelLabel: 'Cancel' },
      )
      if (!yes) return null
      return deleteImages([id])
    },
    onSuccess: (res) => {
      if (!res) return
      if (res.failed.length > 0) {
        alert(
          `Some files could not be deleted:\n\n` +
            res.failed.map((f) => `• ${f.path}\n  ${f.error}`).join('\n')
        )
      }
      clearSelection()
      qc.invalidateQueries({ queryKey: ['images'] })
      qc.invalidateQueries({ queryKey: ['folders'] })
      qc.invalidateQueries({ queryKey: ['tags'] })
    },
  })

  if (q.isLoading || !q.data) {
    return (
      <aside className="border-l border-surface-border p-4 h-full min-h-0">
        <TitleBar label="Details" onClose={onClose} />
        <div className="text-slate-500 text-sm">Loading…</div>
      </aside>
    )
  }

  const d = q.data

  return (
    <aside className="border-l border-surface-border p-4 h-full min-h-0 overflow-y-auto">
      <TitleBar
        label={saving ? `Saving ${saving}…` : 'Details'}
        onClose={onClose}
      />

      <div className="bg-black/50 rounded-lg overflow-hidden aspect-video grid place-items-center mb-4">
        <img
          src={toAssetUrl(d.path)}
          alt={d.filename}
          className="max-w-full max-h-full object-contain"
        />
      </div>

      <Field label="Filename">
        <div className="text-sm text-slate-300 truncate" title={d.filename}>{d.filename}</div>
      </Field>

      <Field label="Path">
        <div
          className="text-[11px] text-slate-500 truncate cursor-text select-text"
          title={d.path}
        >
          {d.path}
        </div>
      </Field>

      <Field label="Rating">
        <StarRating
          value={rating}
          onChange={(v) => {
            setRating(v)
            void applyPatch({ rating: v }, 'rating')
          }}
        />
      </Field>

      <Field label="Title">
        <input
          className="input"
          placeholder="(no title)"
          value={title}
          onChange={(e) => {
            setTitle(e.target.value)
            debouncedSaveTitle(e.target.value)
          }}
          onBlur={() => {
            void applyPatch({ title: title.trim() === '' ? null : title }, 'title')
          }}
          onKeyDown={(e) => {
            if (e.key === 'Enter') (e.target as HTMLInputElement).blur()
          }}
        />
      </Field>

      <Field label="Tags">
        <TagInput
          tags={tags}
          onChange={(next) => {
            setTags(next)
            void applyPatch({ tags: next }, 'tags')
          }}
        />
      </Field>

      <Field label="Comment">
        <textarea
          className="textarea"
          rows={3}
          placeholder="(no comment)"
          value={comment}
          onChange={(e) => {
            setComment(e.target.value)
            debouncedSaveComment(e.target.value)
          }}
          onBlur={() => {
            void applyPatch(
              { comment: comment.trim() === '' ? null : comment },
              'comment',
            )
          }}
        />
      </Field>

      <Meta d={d} />

      <div className="mt-4 pt-3 border-t border-surface-border">
        <button
          className="w-full inline-flex items-center justify-center gap-2 px-3 py-2 rounded-md border border-red-500/40 text-red-300 hover:bg-red-500/15 hover:border-red-500/70 text-sm transition-colors"
          onClick={() => deleteMutation.mutate()}
          disabled={deleteMutation.isPending}
          title="Move file to the Recycle Bin"
        >
          <TrashIcon />
          {deleteMutation.isPending ? 'Deleting…' : 'Delete image'}
        </button>
        <div className="text-[11px] text-slate-500 mt-1 text-center">
          Moves to Recycle Bin. Sidecar .xmp is also removed.
        </div>
      </div>
    </aside>
  )
}

function Meta({ d }: { d: ImageDetails }) {
  const rows: [string, string | number | null][] = [
    ['Dimensions', d.width && d.height ? `${d.width} × ${d.height}` : null],
    ['Size', formatBytes(d.sizeBytes)],
    ['Taken', d.takenAt ? new Date(d.takenAt).toLocaleString() : null],
    ['Modified', new Date(d.mtimeMs).toLocaleString()],
    ['Camera',
      [d.cameraMake, d.cameraModel].filter(Boolean).join(' ') || null],
    ['Format', d.ext.toUpperCase()],
    ['Metadata saved',
      d.metaWrittenAt ? new Date(d.metaWrittenAt).toLocaleString() : 'never'],
  ]
  return (
    <div className="mt-3 border-t border-surface-border pt-3">
      <div className="text-[11px] uppercase tracking-wider text-slate-500 mb-1">
        File info
      </div>
      <dl className="grid grid-cols-[110px_1fr] gap-y-1 text-xs">
        {rows.map(([k, v]) => (
          <div key={k} className="contents">
            <dt className="text-slate-500">{k}</dt>
            <dd className="text-slate-300 truncate" title={String(v ?? '')}>
              {v ?? <span className="text-slate-600">—</span>}
            </dd>
          </div>
        ))}
      </dl>
    </div>
  )
}

function MultiDetails({ ids, onClose }: { ids: number[]; onClose: () => void }) {
  const qc = useQueryClient()
  const clearSelection = useStore((s) => s.clearSelection)
  const [tagsAdd, setTagsAdd] = useState<string[]>([])
  const [tagsRemove, setTagsRemove] = useState<string[]>([])
  // Refs mirror the state so applyTags.mutationFn sees the LATEST values even
  // if the click handler fires in the same React batch as the input's onBlur.
  const tagsAddRef = useRef<string[]>([])
  const tagsRemoveRef = useRef<string[]>([])
  tagsAddRef.current = tagsAdd
  tagsRemoveRef.current = tagsRemove
  const [lastResult, setLastResult] = useState<string | null>(null)

  const invalidateAll = (updatedIds: number[]) => {
    qc.invalidateQueries({ queryKey: ['images'] })
    qc.invalidateQueries({ queryKey: ['tags'] })
    qc.invalidateQueries({ queryKey: ['folders'] })
    // Drop cached per-image details for anything we edited so that a later
    // single-image selection refetches instead of showing stale values.
    for (const id of updatedIds) {
      qc.invalidateQueries({ queryKey: ['image', id] })
    }
  }

  const setRating = useMutation({
    mutationFn: (v: number | null) => batchUpdateMetadata(ids, { rating: v }),
    onSuccess: (updatedIds) => invalidateAll(updatedIds ?? []),
  })

  const applyTags = useMutation({
    mutationFn: async () => {
      const add = tagsAddRef.current
      const remove = tagsRemoveRef.current
      logFrontend(
        'info',
        `applyTags dispatch: ids=${ids.length} add=[${add.join(',')}] remove=[${remove.join(',')}]`,
      )
      if (add.length === 0 && remove.length === 0) {
        logFrontend('warn', 'applyTags: nothing to save (add + remove both empty)')
        return [] as number[]
      }
      const result = await batchUpdateMetadata(ids, {
        tagsAdd: add.length ? add : undefined,
        tagsRemove: remove.length ? remove : undefined,
      })
      logFrontend('info', `applyTags: backend returned ${result.length} updated ids`)
      return result
    },
    onSuccess: (updatedIds) => {
      const n = updatedIds?.length ?? 0
      setTagsAdd([])
      setTagsRemove([])
      invalidateAll(updatedIds ?? [])
      setLastResult(n === 0 ? 'Nothing to save' : `Updated ${n} image${n === 1 ? '' : 's'}`)
      window.setTimeout(() => setLastResult(null), 2500)
    },
    onError: (err) => {
      console.error('batch tag save failed', err)
      logFrontend('error', `applyTags failed: ${(err as Error).message}`)
      setLastResult(`Save failed: ${(err as Error).message}`)
    },
  })

  const deleteMutation = useMutation({
    mutationFn: async () => {
      const yes = await confirm(
        `Move ${ids.length} images to the Recycle Bin?\n\nYou can restore them from the Windows Recycle Bin afterwards.`,
        { title: 'Delete images', kind: 'warning', okLabel: 'Move to Recycle Bin', cancelLabel: 'Cancel' },
      )
      if (!yes) return null
      return deleteImages(ids)
    },
    onSuccess: (res) => {
      if (!res) return
      if (res.failed.length > 0) {
        alert(
          `${res.deleted.length} deleted, ${res.failed.length} failed.\n\n` +
            res.failed.slice(0, 10).map((f) => `• ${f.path}\n  ${f.error}`).join('\n')
        )
      }
      clearSelection()
      qc.invalidateQueries({ queryKey: ['images'] })
      qc.invalidateQueries({ queryKey: ['folders'] })
      qc.invalidateQueries({ queryKey: ['tags'] })
    },
  })

  return (
    <aside className="border-l border-surface-border p-4 h-full min-h-0 overflow-y-auto">
      <TitleBar label={`${ids.length} images selected`} onClose={onClose} />

      <Field label="Set rating">
        <StarRating value={null} onChange={(v) => setRating.mutate(v)} />
        <div className="text-[11px] text-slate-500 mt-1">
          Click a star to apply to all selected images.
        </div>
      </Field>

      <Field label="Add tags">
        <TagInput
          tags={tagsAdd}
          onChange={(next) => {
            // Also imperatively update the ref so that if the user's click
            // on Apply is batched with this onChange (triggered by input
            // blur), the mutation still sees the newly-committed tag.
            tagsAddRef.current = next
            setTagsAdd(next)
          }}
        />
      </Field>

      <Field label="Remove tags">
        <TagInput
          tags={tagsRemove}
          onChange={(next) => {
            tagsRemoveRef.current = next
            setTagsRemove(next)
          }}
        />
      </Field>

      <button
        className="btn-primary mt-2"
        disabled={
          applyTags.isPending ||
          (tagsAdd.length === 0 && tagsRemove.length === 0)
        }
        onClick={() => applyTags.mutate()}
      >
        {applyTags.isPending
          ? `Saving ${ids.length} images…`
          : `Apply tag changes to ${ids.length} images`}
      </button>
      {lastResult && (
        <div
          className={`mt-2 text-xs ${
            lastResult.startsWith('Save failed')
              ? 'text-red-400'
              : 'text-emerald-400'
          }`}
        >
          {lastResult}
        </div>
      )}

      <div className="mt-6 pt-3 border-t border-surface-border">
        <button
          className="w-full inline-flex items-center justify-center gap-2 px-3 py-2 rounded-md border border-red-500/40 text-red-300 hover:bg-red-500/15 hover:border-red-500/70 text-sm transition-colors"
          onClick={() => deleteMutation.mutate()}
          disabled={deleteMutation.isPending}
        >
          <TrashIcon />
          {deleteMutation.isPending ? 'Deleting…' : `Delete ${ids.length} images`}
        </button>
        <div className="text-[11px] text-slate-500 mt-1 text-center">
          Moves files to Recycle Bin. Sidecar .xmp files are also removed.
        </div>
      </div>
    </aside>
  )
}

function Field({
  label,
  children,
}: {
  label: string
  children: React.ReactNode
}) {
  return (
    <div className="mb-3">
      <div className="text-[11px] uppercase tracking-wider text-slate-500 mb-1">
        {label}
      </div>
      {children}
    </div>
  )
}

function formatBytes(n: number) {
  if (n < 1024) return `${n} B`
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`
  if (n < 1024 * 1024 * 1024) return `${(n / 1024 / 1024).toFixed(1)} MB`
  return `${(n / 1024 / 1024 / 1024).toFixed(2)} GB`
}

function TrashIcon() {
  return (
    <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M3 6h18M8 6V4a2 2 0 012-2h4a2 2 0 012 2v2M19 6l-1 14a2 2 0 01-2 2H8a2 2 0 01-2-2L5 6" />
    </svg>
  )
}
