import { useEffect } from 'react'
import { useQueryClient } from '@tanstack/react-query'
import { confirm } from '@tauri-apps/plugin-dialog'
import { TopBar } from './features/TopBar'
import { Sidebar } from './features/Sidebar'
import { ImageGrid } from './features/ImageGrid'
import { DetailsPanel } from './features/DetailsPanel'
import { StatusBar } from './features/StatusBar'
import { deleteImages } from './ipc'
import { useStore } from './store'

export default function App() {
  const detailsOpen = useStore((s) => s.detailsOpen)
  const qc = useQueryClient()

  // Global keyboard shortcuts.
  useEffect(() => {
    const handler = async (e: KeyboardEvent) => {
      const target = e.target as HTMLElement | null
      const inField =
        target instanceof HTMLInputElement ||
        target instanceof HTMLTextAreaElement ||
        target?.getAttribute?.('contenteditable') === 'true'

      if (inField) return

      const sel = useStore.getState().selection

      if (e.key === 'Escape') {
        useStore.getState().clearSelection()
        return
      }

      if (e.key === 'Delete' && sel.ids.size > 0) {
        e.preventDefault()
        const ids = [...sel.ids]
        const yes = await confirm(
          ids.length === 1
            ? 'Move this file to the Recycle Bin?'
            : `Move ${ids.length} files to the Recycle Bin?`,
          {
            title: 'Delete',
            kind: 'warning',
            okLabel: 'Move to Recycle Bin',
            cancelLabel: 'Cancel',
          }
        )
        if (!yes) return
        const res = await deleteImages(ids)
        if (res.failed.length > 0) {
          alert(
            `${res.deleted.length} deleted, ${res.failed.length} failed.\n\n` +
              res.failed
                .slice(0, 10)
                .map((f) => `• ${f.path}\n  ${f.error}`)
                .join('\n')
          )
        }
        useStore.getState().clearSelection()
        qc.invalidateQueries({ queryKey: ['images'] })
        qc.invalidateQueries({ queryKey: ['folders'] })
        qc.invalidateQueries({ queryKey: ['tags'] })
        return
      }

    }
    window.addEventListener('keydown', handler)
    return () => window.removeEventListener('keydown', handler)
  }, [qc])

  return (
    <div className="h-full grid grid-rows-[auto_1fr_auto] bg-surface text-slate-200">
      <TopBar />
      <div
        className="min-h-0 grid"
        style={{
          gridTemplateColumns: detailsOpen ? '260px 1fr 340px' : '260px 1fr',
        }}
      >
        <Sidebar />
        <ImageGrid />
        {detailsOpen && <DetailsPanel />}
      </div>
      <StatusBar />
    </div>
  )
}
