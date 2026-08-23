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
          Select a file to edit its metadata.
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

  // Only editable field the user drives directly from this panel is the title;
  // tags have their own controlled component. We reset on id-change so switching
  // the selection loads the newly-selected file's values.
  const [title, setTitle] = useState('')
  const [tags, setTags] = useState<string[]>([])
  const [saving, setSaving] = useState<string | null>(null)
  const [saveError, setSaveError] = useState<string | null>(null)

  const lastLoadedId = useRef<number | null>(null)

  useEffect(() => {
    if (!q.data) return
    if (lastLoadedId.current === q.data.id) return
    lastLoadedId.current = q.data.id
    setTitle(q.data.title ?? '')
    setTags(q.data.tags)
    setSaveError(null)
  }, [q.data])

  const applyPatch = async (patch: MetadataPatch, label: string) => {
    setSaving(label)
    try {
      const updated = await updateImageMetadata(id, patch)
      qc.setQueryData(['image', id], updated)
      qc.invalidateQueries({ queryKey: ['images'] })
      qc.invalidateQueries({ queryKey: ['tags'] })
      setSaveError(null)
    } catch (e) {
      console.error('metadata save failed', e)
      const msg =
        e instanceof Error
          ? e.message
          : typeof e === 'string'
            ? e
            : JSON.stringify(e)
      setSaveError(msg)
    } finally {
      setSaving(null)
    }
  }

  const debouncedSaveTitle = useDebouncedCallback((v: string) => {
    void applyPatch({ title: v.trim() === '' ? null : v }, 'title')
  }, 600)

  const deleteMutation = useMutation({
    mutationFn: async () => {
      const yes = await confirm(
        `Move "${q.data?.filename ?? 'this file'}" to the Recycle Bin?\n\nYou can restore it from the Windows Recycle Bin afterwards.`,
        { title: 'Delete file', kind: 'warning', okLabel: 'Move to Recycle Bin', cancelLabel: 'Cancel' },
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
  const isImage = isImagePreviewable(d.ext)

  return (
    <aside className="border-l border-surface-border p-4 h-full min-h-0 overflow-y-auto">
      <TitleBar
        label={saving ? `Saving ${saving}…` : 'Details'}
        onClose={onClose}
      />

      <div className="bg-black/50 rounded-lg overflow-hidden aspect-video mb-4 relative">
        {isImage ? (
          <img
            src={toAssetUrl(d.path)}
            alt={d.filename}
            className="absolute inset-0 w-full h-full object-contain"
          />
        ) : (
          <div className="absolute inset-0 grid place-items-center text-center text-slate-500 px-4">
            <div>
              <div className="text-4xl mb-1">📄</div>
              <div className="text-xs uppercase tracking-wider">
                {d.ext.toUpperCase()}
              </div>
              <div className="text-[11px] mt-1">No inline preview</div>
            </div>
          </div>
        )}
      </div>

      {/* -------- Section 1: editable title -------- */}
      <Section label="Title">
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
      </Section>

      {/* -------- Section 2: editable tags -------- */}
      <Section label="Tags">
        <TagInput
          tags={tags}
          onChange={(next) => {
            setTags(next)
            void applyPatch({ tags: next }, 'tags')
          }}
        />
        <div className="text-[11px] text-slate-500 mt-1">
          Saved in this folder's Magpie library. The original file is not
          modified.
        </div>
        {saveError && (
          <div className="text-[11px] text-red-300 mt-1 whitespace-pre-wrap">
            Save failed: {saveError}
          </div>
        )}
      </Section>

      {/* -------- Section 3: format-specific editable metadata --------
       *
       * We currently expose title + tags for every writable handler.
       * Format-specific editable metadata (e.g. GPS, description) will land
       * in this section as those handlers grow their surface area. For now
       * we show the format handler name so users know which pipeline is
       * active. */}
      <Section label="Format metadata">
        <div className="text-xs text-slate-400">
          Handler:{' '}
          <span className="text-slate-200">{d.formatHandler}</span>
        </div>
      </Section>

      {/* -------- Section 4: read-only file info -------- */}
      <Section label="File info">
        <ReadOnlyList d={d} />
      </Section>

      <div className="mt-4 pt-3 border-t border-surface-border">
        <button
          className="w-full inline-flex items-center justify-center gap-2 px-3 py-2 rounded-md border border-red-500/40 text-red-300 hover:bg-red-500/15 hover:border-red-500/70 text-sm transition-colors"
          onClick={() => deleteMutation.mutate()}
          disabled={deleteMutation.isPending}
          title="Move file to the Recycle Bin"
        >
          <TrashIcon />
          {deleteMutation.isPending ? 'Deleting…' : 'Delete file'}
        </button>
        <div className="text-[11px] text-slate-500 mt-1 text-center">
          Moves to Recycle Bin. Tags and title are also cleared from the
          library.
        </div>
      </div>
    </aside>
  )
}

function ReadOnlyList({ d }: { d: ImageDetails }) {
  const baseRows: [string, string | null][] = [
    ['Filename', d.filename],
    ['Path', d.path],
    ['Size', formatBytes(d.sizeBytes)],
    ['Format', d.ext.toUpperCase()],
    ['Modified', new Date(d.mtimeMs).toLocaleString()],
    ['Imported', new Date(d.importedAt).toLocaleString()],
  ]
  const techRows: [string, string | null][] = (d.technical ?? []).map(
    ([k, v]) => [k, v],
  )
  const rows = [...baseRows, ...techRows]
  return (
    <dl className="grid grid-cols-[130px_1fr] gap-y-1 text-xs">
      {rows.map(([k, v], i) => (
        <div key={`${k}-${i}`} className="contents">
          <dt className="text-slate-500">{k}</dt>
          <dd className="text-slate-300 truncate" title={String(v ?? '')}>
            {v ?? <span className="text-slate-600">—</span>}
          </dd>
        </div>
      ))}
    </dl>
  )
}

function MultiDetails({ ids, onClose }: { ids: number[]; onClose: () => void }) {
  const qc = useQueryClient()
  const clearSelection = useStore((s) => s.clearSelection)
  const [tagsAdd, setTagsAdd] = useState<string[]>([])
  const [tagsRemove, setTagsRemove] = useState<string[]>([])
  const tagsAddRef = useRef<string[]>([])
  const tagsRemoveRef = useRef<string[]>([])
  tagsAddRef.current = tagsAdd
  tagsRemoveRef.current = tagsRemove
  const [lastResult, setLastResult] = useState<string | null>(null)

  const invalidateAll = (updatedIds: number[]) => {
    qc.invalidateQueries({ queryKey: ['images'] })
    qc.invalidateQueries({ queryKey: ['tags'] })
    qc.invalidateQueries({ queryKey: ['folders'] })
    for (const id of updatedIds) {
      qc.invalidateQueries({ queryKey: ['image', id] })
    }
  }

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
      setLastResult(n === 0 ? 'Nothing to save' : `Updated ${n} file${n === 1 ? '' : 's'}`)
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
        `Move ${ids.length} files to the Recycle Bin?\n\nYou can restore them from the Windows Recycle Bin afterwards.`,
        { title: 'Delete files', kind: 'warning', okLabel: 'Move to Recycle Bin', cancelLabel: 'Cancel' },
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
      <TitleBar label={`${ids.length} files selected`} onClose={onClose} />

      <Section label="Add tags">
        <TagInput
          tags={tagsAdd}
          onChange={(next) => {
            tagsAddRef.current = next
            setTagsAdd(next)
          }}
        />
      </Section>

      <Section label="Remove tags">
        <TagInput
          tags={tagsRemove}
          onChange={(next) => {
            tagsRemoveRef.current = next
            setTagsRemove(next)
          }}
        />
      </Section>

      <button
        className="btn-primary mt-2"
        disabled={
          applyTags.isPending ||
          (tagsAdd.length === 0 && tagsRemove.length === 0)
        }
        onClick={() => applyTags.mutate()}
      >
        {applyTags.isPending
          ? `Saving ${ids.length} files…`
          : `Apply tag changes to ${ids.length} files`}
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
          {deleteMutation.isPending ? 'Deleting…' : `Delete ${ids.length} files`}
        </button>
        <div className="text-[11px] text-slate-500 mt-1 text-center">
          Moves files to Recycle Bin. Tags and titles are also cleared
          from the library.
        </div>
      </div>
    </aside>
  )
}

function Section({
  label,
  children,
}: {
  label: string
  children: React.ReactNode
}) {
  return (
    <div className="mb-4 pb-3 border-b border-surface-border last:border-b-0 last:pb-0 last:mb-3">
      <div className="text-[11px] uppercase tracking-wider text-slate-500 mb-1.5">
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

/**
 * Extensions we can render inline via the OS's decoder (Tauri's asset://
 * protocol serves the raw file to <img>, so this is really "extensions
 * modern browsers can decode"). Everything else gets a placeholder tile.
 */
function isImagePreviewable(ext: string): boolean {
  const e = ext.toLowerCase().replace(/^\./, '')
  return ['jpg', 'jpeg', 'png', 'webp', 'gif', 'bmp', 'svg', 'avif'].includes(e)
}

function TrashIcon() {
  return (
    <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M3 6h18M8 6V4a2 2 0 012-2h4a2 2 0 012 2v2M19 6l-1 14a2 2 0 01-2 2H8a2 2 0 01-2-2L5 6" />
    </svg>
  )
}
