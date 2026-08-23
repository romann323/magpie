import { useState } from 'react'
import { open, confirm } from '@tauri-apps/plugin-dialog'
import { useMutation, useQueryClient } from '@tanstack/react-query'
import { addLibraryFolder, checkFolderSyncRisk, rescanAll } from '../ipc'
import { useStore } from '../store'
import { PRODUCT_NAME } from '../brand'
import { AppIcon } from '../components/AppIcon'
import type { ImageSort, SortBy, SortDir } from '../types'

export function TopBar() {
  const search = useStore((s) => s.search)
  const setSearch = useStore((s) => s.setSearch)
  const sort = useStore((s) => s.sort)
  const setSort = useStore((s) => s.setSort)
  const detailsOpen = useStore((s) => s.detailsOpen)
  const setDetailsOpen = useStore((s) => s.setDetailsOpen)
  const qc = useQueryClient()

  const [scanning, setScanning] = useState(false)

  const addFolder = useMutation({
    mutationFn: async () => {
      const picked = await open({
        directory: true,
        multiple: false,
        title: 'Select a folder to add to your library',
      })
      if (!picked) return null
      const path = typeof picked === 'string' ? picked : (picked as string)

      // Cloud-synced or network share? Warn once so the user knows what
      // "open on two PCs" means for the library.
      const risk = await checkFolderSyncRisk(path)
      if (risk) {
        const ok = await confirm(risk.message, {
          title: `Folder is on ${risk.provider}`,
          kind: 'warning',
          okLabel: 'Add anyway',
          cancelLabel: 'Cancel',
        })
        if (!ok) return null
      }

      return addLibraryFolder(path)
    },
    onSuccess: (result) => {
      if (result) {
        qc.invalidateQueries({ queryKey: ['folders'] })
      }
    },
  })

  const rescan = useMutation({
    mutationFn: async () => {
      setScanning(true)
      try {
        return await rescanAll()
      } finally {
        setScanning(false)
        qc.invalidateQueries({ queryKey: ['images'] })
        qc.invalidateQueries({ queryKey: ['folders'] })
        qc.invalidateQueries({ queryKey: ['tags'] })
      }
    },
  })

  return (
    <header className="h-12 border-b border-surface-border flex items-center gap-3 px-3 select-none">
      <div className="flex items-center gap-2 pr-3 border-r border-surface-border">
        <AppIcon className="w-6 h-6 rounded shrink-0" />
        <span className="font-semibold tracking-tight">{PRODUCT_NAME}</span>
      </div>

      <button
        className="btn-primary"
        onClick={() => addFolder.mutate()}
        disabled={addFolder.isPending}
      >
        <PlusIcon />
        Add folder
      </button>
      <button
        className="btn"
        onClick={() => rescan.mutate()}
        disabled={scanning || rescan.isPending}
        title="Rescan all folders"
      >
        <RefreshIcon spinning={scanning || rescan.isPending} />
        Rescan
      </button>

      <div className="flex-1 max-w-xl mx-4">
        <input
          type="search"
          className="input"
          placeholder="Search title, filename, tags…"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
        />
      </div>

      <div className="flex items-center gap-1 text-slate-400 text-xs">
        <span>Sort:</span>
        <select
          className="input py-1"
          style={{ width: 'auto' }}
          value={sort.by}
          onChange={(e) =>
            setSort({ ...sort, by: e.target.value as SortBy } as ImageSort)
          }
        >
          <option value="takenAt">Taken</option>
          <option value="filename">Filename</option>
          <option value="addedAt">Added</option>
          <option value="size">Size</option>
        </select>
        <button
          className="btn-icon"
          title={sort.dir === 'asc' ? 'Ascending' : 'Descending'}
          onClick={() =>
            setSort({
              ...sort,
              dir: (sort.dir === 'asc' ? 'desc' : 'asc') as SortDir,
            })
          }
        >
          {sort.dir === 'asc' ? <ArrowUpIcon /> : <ArrowDownIcon />}
        </button>
      </div>

      <button
        className="btn-icon"
        onClick={() => setDetailsOpen(!detailsOpen)}
        title={detailsOpen ? 'Hide details' : 'Show details'}
      >
        <SidebarRightIcon active={detailsOpen} />
      </button>
    </header>
  )
}

function PlusIcon() {
  return (
    <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M12 5v14M5 12h14" />
    </svg>
  )
}
function RefreshIcon({ spinning }: { spinning: boolean }) {
  return (
    <svg
      viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"
      style={{ animation: spinning ? 'spin 1s linear infinite' : undefined }}
    >
      <path d="M23 4v6h-6M1 20v-6h6" />
      <path d="M3.51 9a9 9 0 0114.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0020.49 15" />
    </svg>
  )
}
function ArrowUpIcon() {
  return <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M12 19V5M5 12l7-7 7 7" /></svg>
}
function ArrowDownIcon() {
  return <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M12 5v14M19 12l-7 7-7-7" /></svg>
}
function SidebarRightIcon({ active }: { active: boolean }) {
  return (
    <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke={active ? 'currentColor' : 'currentColor'} strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <rect x="3" y="3" width="18" height="18" rx="2" />
      <path d="M15 3v18" />
    </svg>
  )
}
