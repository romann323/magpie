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
              className="flex-1"
              label={folderLabel(f.path)}
              badge={f.imageCount ? String(f.imageCount) : undefined}
              active={view.kind === 'folder' && view.id === f.id}
              onClick={() => setView({ kind: 'folder', id: f.id })}
              title={f.path}
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

      <SidebarSection title="Tags">
        {(tags.data?.length ?? 0) === 0 && (
          <div className="text-xs text-slate-500 px-2 py-1">No tags yet.</div>
        )}
        {tags.data?.slice(0, 40).map((t) => (
          <SidebarItem
            key={t.name}
            label={t.name}
            badge={String(t.count)}
            active={view.kind === 'tag' && view.name === t.name}
            onClick={() => setView({ kind: 'tag', name: t.name })}
          />
        ))}
      </SidebarSection>
    </aside>
  )
}

function SidebarSection({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div className="mb-4">
      <div className="px-2 pb-1 text-[11px] uppercase tracking-wider text-slate-500">
        {title}
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
