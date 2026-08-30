import { useEffect } from 'react'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import {
  listLibraryFolders,
  listTags,
  removeLibraryFolder,
  onScanProgress,
} from '../ipc'
import { useStore } from '../store'
import clsx from 'clsx'

export function Sidebar() {
  const view = useStore((s) => s.view)
  const setView = useStore((s) => s.setView)
  const selectedTags = useStore((s) => s.selectedTags)
  const toggleSelectedTag = useStore((s) => s.toggleSelectedTag)
  const clearSelectedTags = useStore((s) => s.clearSelectedTags)
  const qc = useQueryClient()

  const folders = useQuery({
    queryKey: ['folders'],
    queryFn: listLibraryFolders,
    refetchInterval: 10_000,
  })

  const tags = useQuery({
    queryKey: ['tags'],
    queryFn: () => listTags(),
    staleTime: 5_000,
  })

  const removeFolder = useMutation({
    mutationFn: (id: number) => removeLibraryFolder(id),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['folders'] })
      qc.invalidateQueries({ queryKey: ['images'] })
      qc.invalidateQueries({ queryKey: ['tags'] })
    },
  })

  useEffect(() => {
    const un = onScanProgress((p) => {
      if (p.finished) {
        qc.invalidateQueries({ queryKey: ['images'] })
        qc.invalidateQueries({ queryKey: ['folders'] })
        qc.invalidateQueries({ queryKey: ['tags'] })
      }
    })
    return () => {
      un.then((fn) => fn()).catch(() => {})
    }
  }, [qc])

  const selectedSet = new Set(selectedTags.map((t) => t.toLowerCase()))
  const anyTagSelected = selectedTags.length > 0

  return (
    <aside className="min-h-0 h-full border-r border-surface-border overflow-y-auto py-3 px-2">
      <SidebarSection title="Library">
        <SidebarItem
          label="All photos"
          active={view.kind === 'all'}
          onClick={() => setView({ kind: 'all' })}
        />
      </SidebarSection>

      <SidebarSection title="Folders">
        {folders.data && folders.data.length === 0 && (
          <div className="text-xs text-slate-500 px-2 py-1">
            No folders yet. Use “Add folder” to get started.
          </div>
        )}
        {folders.data?.map((f) => (
          <div key={f.id} className="group flex items-center">
            <SidebarItem
              className={clsx('flex-1', !f.isAvailable && 'opacity-60')}
              label={
                f.isAvailable ? (
                  folderLabel(f.path)
                ) : (
                  <span title="Folder is not reachable right now">
                    {folderLabel(f.path)}{' '}
                    <span className="text-amber-400">(offline)</span>
                  </span>
                )
              }
              badge={f.imageCount ? String(f.imageCount) : undefined}
              active={view.kind === 'folder' && view.id === f.id}
              onClick={() => setView({ kind: 'folder', id: f.id })}
              title={
                f.isAvailable
                  ? f.path
                  : `${f.path}\n\nThis folder isn't reachable right now. Reconnect the drive and rescan to bring it back.`
              }
            />
            <button
              className="btn-icon opacity-0 group-hover:opacity-100 shrink-0"
              title="Remove folder"
              onClick={(e) => {
                e.stopPropagation()
                if (confirm(`Remove folder from library?\n\n${f.path}\n\n(The files on disk are not deleted.)`)) {
                  removeFolder.mutate(f.id)
                }
              }}
            >
              <TrashIcon />
            </button>
          </div>
        ))}
      </SidebarSection>

      <SidebarSection
        title="Tags"
        action={
          anyTagSelected ? (
            <button
              className="text-[11px] text-slate-400 hover:text-slate-100 uppercase tracking-wider"
              onClick={clearSelectedTags}
              title="Deselect every tag"
            >
              Clear all
            </button>
          ) : undefined
        }
      >
        {(tags.data?.length ?? 0) === 0 && (
          <div className="text-xs text-slate-500 px-2 py-1">No tags yet.</div>
        )}
        {(tags.data?.length ?? 0) > 0 && (
          <div className="flex flex-wrap gap-1.5 px-2 pt-1 pb-1">
            {tags.data!.slice(0, 200).map((t) => {
              const checked = selectedSet.has(t.name.toLowerCase())
              return (
                <TagBubble
                  key={t.name}
                  name={t.name}
                  count={t.count}
                  selected={checked}
                  onToggle={() => toggleSelectedTag(t.name)}
                />
              )
            })}
          </div>
        )}
      </SidebarSection>
    </aside>
  )
}

function TagBubble({
  name,
  count,
  selected,
  onToggle,
}: {
  name: string
  count: number
  selected: boolean
  onToggle: () => void
}) {
  return (
    <button
      type="button"
      onClick={onToggle}
      aria-pressed={selected}
      title={`${name} (${count})`}
      className={clsx(
        'inline-flex items-center gap-1 max-w-full',
        'rounded-full border px-2.5 py-0.5 text-xs leading-5',
        'transition-colors cursor-pointer',
        selected
          ? 'bg-accent text-white border-accent hover:bg-accent-hover'
          : 'bg-surface-raised text-slate-300 border-surface-border hover:bg-surface-hover hover:text-slate-100',
      )}
    >
      <span className="truncate">{name}</span>
      <span
        className={clsx(
          'tabular-nums shrink-0 text-[10px]',
          selected ? 'text-white/80' : 'text-slate-500',
        )}
      >
        {count}
      </span>
    </button>
  )
}

function SidebarSection({
  title,
  action,
  children,
}: {
  title: string
  action?: React.ReactNode
  children: React.ReactNode
}) {
  return (
    <div className="mb-4">
      <div className="flex items-center justify-between px-2 pb-1">
        <div className="text-[11px] uppercase tracking-wider text-slate-500">
          {title}
        </div>
        {action}
      </div>
      <div className="flex flex-col gap-0.5">{children}</div>
    </div>
  )
}

function SidebarItem({
  label,
  active,
  badge,
  onClick,
  className,
  title,
}: {
  label: React.ReactNode
  active?: boolean
  badge?: string
  onClick: () => void
  className?: string
  title?: string
}) {
  return (
    <button
      className={clsx('sidebar-item', active && 'active', className)}
      onClick={onClick}
      title={title}
    >
      <span className="truncate flex-1 text-left">{label}</span>
      {badge && (
        <span className="text-[11px] text-slate-500 tabular-nums">{badge}</span>
      )}
    </button>
  )
}

function folderLabel(path: string): string {
  const norm = path.replace(/\\/g, '/')
  const parts = norm.split('/').filter(Boolean)
  return parts.at(-1) ?? path
}

function TrashIcon() {
  return (
    <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M3 6h18M8 6V4a2 2 0 012-2h4a2 2 0 012 2v2M19 6l-1 14a2 2 0 01-2 2H8a2 2 0 01-2-2L5 6" />
    </svg>
  )
}
