import { useEffect, useMemo, useRef, useState } from 'react'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { confirm } from '@tauri-apps/plugin-dialog'
import {
  batchUpdateMetadata,
  deleteImages,
  getImage,
  logFrontend,
  renameImage,
  toAssetUrl,
  updateImageMetadata,
} from '../ipc'
import { filterFromView, useStore } from '../store'
import type { ImageDetails, MetadataPatch } from '../types'
import { TagInput } from './TagInput'
import { openMagnifierWindow } from './openMagnifierWindow'

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
  const pushUndo = useStore((s) => s.pushUndo)
  const view = useStore((s) => s.view)
  const search = useStore((s) => s.search)
  const sort = useStore((s) => s.sort)
  const extraFilter = useStore((s) => s.extraFilter)
  const selectedTags = useStore((s) => s.selectedTags)

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
  const lastSavedTitle = useRef<string>('')
  const lastSavedTags = useRef<string[]>([])

  useEffect(() => {
    if (!q.data) return
    if (lastLoadedId.current === q.data.id) return
    lastLoadedId.current = q.data.id
    setTitle(q.data.title ?? '')
    lastSavedTitle.current = q.data.title ?? ''
    setTags(q.data.userTags)
    lastSavedTags.current = q.data.userTags
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
      return updated
    } catch (e) {
      console.error('metadata save failed', e)
      const msg =
        e instanceof Error
          ? e.message
          : typeof e === 'string'
            ? e
            : JSON.stringify(e)
      setSaveError(msg)
      throw e
    } finally {
      setSaving(null)
    }
  }

  const commitTitle = async (v: string) => {
    const prev = lastSavedTitle.current
    const next = v.trim() === '' ? '' : v
    if (prev === next) return
    try {
      await applyPatch({ title: next === '' ? null : next }, 'title')
      pushUndo({
        kind: 'title',
        id,
        from: prev === '' ? null : prev,
        to: next === '' ? null : next,
      })
      lastSavedTitle.current = next
    } catch {
      /* error surfaced via saveError */
    }
  }

  const debouncedSaveTitle = useDebouncedCallback((v: string) => {
    void commitTitle(v)
  }, 600)

  const commitTags = async (next: string[]) => {
    const prev = lastSavedTags.current
    if (sameStrings(prev, next)) return
    try {
      await applyPatch({ tags: next }, 'tags')
      pushUndo({ kind: 'tags', id, from: prev, to: next })
      lastSavedTags.current = next
    } catch {
      /* error surfaced via saveError */
    }
  }

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

      <div
        className="bg-black/50 rounded-lg overflow-hidden aspect-video mb-4 relative cursor-zoom-in"
        onDoubleClick={() =>
          void openMagnifierWindow(
            id,
            filterFromView(view, extraFilter, search, selectedTags),
            sort,
          )
        }
        title="Double-click to open magnifier"
      >
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
            void commitTitle(title)
          }}
          onKeyDown={(e) => {
            if (e.key === 'Enter') (e.target as HTMLInputElement).blur()
          }}
        />
      </Section>

      {/* -------- Section 2: editable user tags -------- */}
      <Section
        label="Your tags"
        icon={<PencilIcon />}
        titleTooltip="Editable. Saved in the Magpie project database — the source file is not touched."
      >
        <TagInput
          tags={tags}
          onChange={(next) => {
            setTags(next)
            void commitTags(next)
          }}
        />
        <div className="text-[11px] text-slate-500 mt-1">
          Saved in the Magpie project database. The source file is not modified.
        </div>
        {saveError && (
          <div className="text-[11px] text-red-300 mt-1 whitespace-pre-wrap">
            Save failed: {saveError}
          </div>
        )}
      </Section>

      {/* -------- Section 3: read-only automatic tags --------
        Always rendered — even when the list is empty — so the
        distinction between user-editable and read-only tags stays
        visible on every file, not just the ones that happen to
        carry embedded metadata. */}
      <Section
        label="Automatic tags"
        icon={<LockIcon />}
        titleTooltip="Read-only. Imported from the file's own metadata (or added by Magpie's auto-tagger). Edit the file in the tool that wrote them and rescan to change these."
      >
        {d.autoTags.length > 0 ? (
          <ReadOnlyTagList tags={d.autoTags} />
        ) : (
          <div className="min-h-[38px] w-full px-2 py-1 rounded-md bg-surface-raised/40 border border-dashed border-surface-border flex items-center text-[11px] text-slate-500 italic cursor-not-allowed">
            No automatic tags on this file.
          </div>
        )}
        <div className="text-[11px] text-slate-500 mt-1">
          Read from the file itself when Magpie scanned it. To change
          them, edit the file in the tool that wrote them and rescan.
        </div>
      </Section>

      {/* -------- Section 3: format-specific editable metadata -------- */}
      <Section label="Format metadata">
        <div className="text-xs text-slate-400">
          Handler:{' '}
          <span className="text-slate-200">{d.formatHandler}</span>
        </div>
      </Section>

      {/* -------- Section 4: read-only file info + editable filename -------- */}
      <Section label="File info">
        <ReadOnlyList d={d} onFilenameSaved={(from, to) => {
          if (from !== to) pushUndo({ kind: 'rename', id, from, to })
        }} />
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

/**
 * Read-only file info list. The filename row alone is editable: Enter
 * commits (via `rename_image`); Escape or blur reverts to the last
 * saved value.
 */
function ReadOnlyList({
  d,
  onFilenameSaved,
}: {
  d: ImageDetails
  onFilenameSaved: (from: string, to: string) => void
}) {
  const qc = useQueryClient()
  const [name, setName] = useState(d.filename)
  const [renameError, setRenameError] = useState<string | null>(null)
  const [renaming, setRenaming] = useState(false)
  const originalRef = useRef(d.filename)

  useEffect(() => {
    setName(d.filename)
    originalRef.current = d.filename
    setRenameError(null)
  }, [d.id, d.filename])

  const commit = async (next: string) => {
    const trimmed = next.trim()
    if (trimmed === originalRef.current) {
      setRenameError(null)
      return
    }
    if (!trimmed) {
      revert()
      return
    }
    setRenaming(true)
    setRenameError(null)
    try {
      const updated = await renameImage(d.id, trimmed)
      qc.setQueryData(['image', d.id], updated)
      qc.invalidateQueries({ queryKey: ['images'] })
      onFilenameSaved(originalRef.current, updated.filename)
      originalRef.current = updated.filename
      setName(updated.filename)
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e)
      setRenameError(msg)
      revert()
    } finally {
      setRenaming(false)
    }
  }

  const revert = () => {
    setName(originalRef.current)
    setRenameError(null)
  }

  const baseRows: { label: string; value: React.ReactNode; title?: string }[] = [
    {
      label: 'Filename',
      value: (
        <div className="flex flex-col gap-1">
          <input
            className="input py-1 text-xs"
            value={name}
            disabled={renaming}
            onChange={(e) => setName(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter') {
                e.preventDefault()
                void commit(name)
                ;(e.target as HTMLInputElement).blur()
              } else if (e.key === 'Escape') {
                e.preventDefault()
                revert()
                ;(e.target as HTMLInputElement).blur()
              }
            }}
            onBlur={() => revert()}
            title="Enter to rename, Esc to revert"
          />
          {renameError && (
            <span className="text-red-300 text-[11px] whitespace-pre-wrap">
              {renameError}
            </span>
          )}
        </div>
      ),
    },
    { label: 'Path', value: d.path, title: d.path },
    { label: 'Size', value: formatBytes(d.sizeBytes) },
    { label: 'Format', value: d.ext.toUpperCase() },
    { label: 'Modified', value: new Date(d.mtimeMs).toLocaleString() },
    { label: 'Imported', value: new Date(d.importedAt).toLocaleString() },
  ]
  const techRows = (d.technical ?? []).map(([k, v]) => ({
    label: k,
    value: v,
    title: v ?? '',
  }))
  const rows = [...baseRows, ...techRows]
  return (
    <dl className="grid grid-cols-[130px_1fr] gap-y-1 text-xs">
      {rows.map((r, i) => (
        <div key={`${r.label}-${i}`} className="contents">
          <dt className="text-slate-500 self-center">{r.label}</dt>
          <dd className="text-slate-300 truncate" title={r.title ?? ''}>
            {r.value ?? <span className="text-slate-600">—</span>}
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

function ReadOnlyTagList({ tags }: { tags: string[] }) {
  return (
    <div
      className="min-h-[38px] w-full px-2 py-1 rounded-md bg-surface-raised/40 border border-dashed border-surface-border flex flex-wrap items-center gap-1 cursor-not-allowed"
      aria-readonly="true"
      title="Read-only — imported from the file's own metadata"
    >
      {tags.map((t) => (
        <span
          key={t}
          className="inline-flex items-center gap-1 px-2 py-0.5 rounded bg-slate-500/10 border border-slate-500/40 text-slate-400 text-xs select-text"
          title="Read-only tag"
        >
          <LockIcon size={10} />
          {t}
        </span>
      ))}
    </div>
  )
}

function Section({
  label,
  children,
  icon,
  titleTooltip,
}: {
  label: string
  children: React.ReactNode
  icon?: React.ReactNode
  titleTooltip?: string
}) {
  return (
    <div className="mb-4 pb-3 border-b border-surface-border last:border-b-0 last:pb-0 last:mb-3">
      <div
        className="text-[11px] uppercase tracking-wider text-slate-500 mb-1.5 flex items-center gap-1.5"
        title={titleTooltip}
      >
        {icon}
        <span>{label}</span>
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

function sameStrings(a: string[], b: string[]): boolean {
  if (a.length !== b.length) return false
  for (let i = 0; i < a.length; i++) {
    if (a[i] !== b[i]) return false
  }
  return true
}

function TrashIcon() {
  return (
    <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M3 6h18M8 6V4a2 2 0 012-2h4a2 2 0 012 2v2M19 6l-1 14a2 2 0 01-2 2H8a2 2 0 01-2-2L5 6" />
    </svg>
  )
}

function LockIcon({ size = 12 }: { size?: number }) {
  return (
    <svg
      viewBox="0 0 24 24"
      width={size}
      height={size}
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <rect x="4" y="11" width="16" height="10" rx="2" />
      <path d="M8 11V7a4 4 0 018 0v4" />
    </svg>
  )
}

function PencilIcon({ size = 12 }: { size?: number }) {
  return (
    <svg
      viewBox="0 0 24 24"
      width={size}
      height={size}
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <path d="M12 20h9" />
      <path d="M16.5 3.5a2.121 2.121 0 013 3L7 19l-4 1 1-4 12.5-12.5z" />
    </svg>
  )
}
