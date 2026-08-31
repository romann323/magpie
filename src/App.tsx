import { useEffect, useMemo, useState } from 'react'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import { confirm, open, save } from '@tauri-apps/plugin-dialog'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { TopBar } from './features/TopBar'
import { Sidebar } from './features/Sidebar'
import { ImageGrid } from './features/ImageGrid'
import { DetailsPanel } from './features/DetailsPanel'
import { StatusBar } from './features/StatusBar'
import { WelcomeScreen } from './features/WelcomeScreen'
import { openMagnifierWindow } from './features/openMagnifierWindow'
import { SettingsDialogs } from './features/SettingsDialogs'
import {
  batchUpdateMetadata,
  closeProject,
  createProject,
  currentProject,
  deleteImages,
  getAppSettings,
  onMenuEvent,
  openProject,
  renameImage,
  saveProjectAs,
  setMenuItemEnabled,
  updateImageMetadata,
} from './ipc'
import { filterFromView, useStore, type UndoEntry } from './store'
import type { AppSettings } from './types'
import { PRODUCT_NAME } from './brand'

export default function App() {
  const detailsOpen = useStore((s) => s.detailsOpen)
  const project = useStore((s) => s.project)
  const setProject = useStore((s) => s.setProject)
  const setSettings = useStore((s) => s.setSettings)
  const settings = useStore((s) => s.settings)
  const qc = useQueryClient()
  const [settingsDialog, setSettingsDialog] = useState<
    'theme' | 'font-size' | 'language' | 'ai-auto-tag' | null
  >(null)

  // -------- Bootstrap: fetch current project + settings on startup ------
  const projectQ = useQuery({
    queryKey: ['current-project'],
    queryFn: currentProject,
  })
  useEffect(() => {
    if (projectQ.data !== undefined) setProject(projectQ.data)
  }, [projectQ.data, setProject])

  const settingsQ = useQuery({
    queryKey: ['app-settings'],
    queryFn: getAppSettings,
  })
  useEffect(() => {
    if (settingsQ.data) setSettings(settingsQ.data)
  }, [settingsQ.data, setSettings])

  // -------- Apply theme + font-size to <html> ---------------------------
  useEffect(() => {
    applyThemeAndFontSize(settings)
  }, [settings])

  // -------- Menu handlers ----------------------------------------------
  useMenuRouter({
    onProjectNew: () => void handleNewProject(qc, setProject),
    onProjectOpen: () => void handleOpenProject(qc, setProject),
    onProjectSave: () => {
      /* SQLite auto-saves; the menu item still exists for symmetry */
      // no-op
    },
    onProjectSaveAs: () => void handleSaveAs(qc, setProject),
    onProjectClose: () => void handleClose(qc, setProject),
    onProjectQuit: () => {
      void getCurrentWindow().close()
    },
    onEditUndo: () => void handleUndoOrRedo('undo', qc),
    onEditRedo: () => void handleUndoOrRedo('redo', qc),
    onViewMagnifier: () => handleOpenMagnifierFromMenu(),
    onSetLanguage: () => setSettingsDialog('language'),
    onSetTheme: () => setSettingsDialog('theme'),
    onSetFontSize: () => setSettingsDialog('font-size'),
    onOpenAiAutoTag: () => setSettingsDialog('ai-auto-tag'),
  })

  // -------- Toggle Edit → Undo/Redo enabled state -----------------------
  const undoLen = useStore((s) => s.undoStack.length)
  const redoLen = useStore((s) => s.redoStack.length)
  useEffect(() => {
    void setMenuItemEnabled('edit_undo', undoLen > 0).catch(() => {})
    void setMenuItemEnabled('edit_redo', redoLen > 0).catch(() => {})
  }, [undoLen, redoLen])

  // -------- Toggle View → Magnifier enabled state based on selection ----
  const selectionPrimary = useStore((s) => s.selection.primary)
  useEffect(() => {
    void setMenuItemEnabled('view_magnifier', selectionPrimary !== null).catch(
      () => {},
    )
  }, [selectionPrimary])

  // -------- Global keyboard shortcuts (Delete + Esc) --------------------
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

  const columnsStyle = useMemo(
    () =>
      ({
        gridTemplateColumns: detailsOpen ? '260px 1fr 340px' : '260px 1fr',
      }) as React.CSSProperties,
    [detailsOpen],
  )

  return (
    <>
      {project ? (
        // Keying on the project path forces a full remount when the
        // user switches projects, so cached asset URLs, virtualised
        // grid state, and Thumbnail image src values all reset
        // cleanly instead of clinging to the previous project.
        <div
          key={project.path}
          className="h-full grid grid-rows-[auto_1fr_auto] bg-surface text-slate-200"
        >
          <TopBar />
          <div className="min-h-0 grid" style={columnsStyle}>
            <Sidebar />
            <ImageGrid />
            {detailsOpen && <DetailsPanel />}
          </div>
          <StatusBar />
        </div>
      ) : (
        <WelcomeScreen />
      )}

      <SettingsDialogs which={settingsDialog} onClose={() => setSettingsDialog(null)} />
    </>
  )
}

// ---------------------------------------------------------------------
//                      Theme + font-size effect
// ---------------------------------------------------------------------

function applyThemeAndFontSize(settings: AppSettings | null) {
  const root = document.documentElement
  // Theme
  const wantsLight = settings?.theme === 'light'
    ? true
    : settings?.theme === 'dark'
      ? false
      : matchMedia('(prefers-color-scheme: light)').matches
  root.classList.toggle('theme-light', wantsLight)
  root.classList.toggle('theme-dark', !wantsLight)
  // Font-size
  root.classList.remove('font-size-small', 'font-size-medium', 'font-size-large')
  const fs = settings?.fontSize ?? 'medium'
  root.classList.add(`font-size-${fs}`)
}

// ---------------------------------------------------------------------
//                        Menu event routing
// ---------------------------------------------------------------------

type MenuHandlers = {
  onProjectNew: () => void
  onProjectOpen: () => void
  onProjectSave: () => void
  onProjectSaveAs: () => void
  onProjectClose: () => void
  onProjectQuit: () => void
  onEditUndo: () => void
  onEditRedo: () => void
  onViewMagnifier: () => void
  onSetLanguage: () => void
  onSetTheme: () => void
  onSetFontSize: () => void
  onOpenAiAutoTag: () => void
}

function useMenuRouter(h: MenuHandlers) {
  useEffect(() => {
    let un: (() => void) | null = null
    onMenuEvent((id) => {
      switch (id) {
        case 'proj_new':
          return h.onProjectNew()
        case 'proj_open':
          return h.onProjectOpen()
        case 'proj_save':
          return h.onProjectSave()
        case 'proj_save_as':
          return h.onProjectSaveAs()
        case 'proj_close':
          return h.onProjectClose()
        case 'proj_quit':
          return h.onProjectQuit()
        case 'edit_undo':
          return h.onEditUndo()
        case 'edit_redo':
          return h.onEditRedo()
        case 'view_magnifier':
          return h.onViewMagnifier()
        case 'set_language':
          return h.onSetLanguage()
        case 'set_theme':
          return h.onSetTheme()
        case 'set_font_size':
          return h.onSetFontSize()
        case 'set_ai_auto_tag':
          return h.onOpenAiAutoTag()
      }
    })
      .then((fn) => {
        un = fn
      })
      .catch(() => {})
    return () => {
      if (un) un()
    }
    // Intentional — we want a fresh subscription only on mount so the
    // ref-based handler picks up latest state each call.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])
}

// ---------------------------------------------------------------------
//                        Menu action helpers
// ---------------------------------------------------------------------

async function handleNewProject(
  qc: ReturnType<typeof useQueryClient>,
  setProject: (p: NonNullable<ReturnType<typeof useStore.getState>['project']>) => void,
) {
  try {
    const picked = await save({
      title: `Create a new ${PRODUCT_NAME} project`,
      defaultPath: 'MyProject.magpie',
      filters: [{ name: `${PRODUCT_NAME} project`, extensions: ['magpie'] }],
    })
    if (!picked) return
    const info = await createProject(picked)
    setProject(info)
    invalidateProjectQueries(qc)
  } catch (e) {
    alert(`Could not create project: ${(e as Error).message}`)
  }
}

async function handleOpenProject(
  qc: ReturnType<typeof useQueryClient>,
  setProject: (p: NonNullable<ReturnType<typeof useStore.getState>['project']>) => void,
) {
  try {
    const picked = await open({
      multiple: false,
      title: `Open a ${PRODUCT_NAME} project`,
      filters: [{ name: `${PRODUCT_NAME} project`, extensions: ['magpie', 'db'] }],
    })
    if (!picked) return
    const p = typeof picked === 'string' ? picked : (picked as string)
    const info = await openProject(p)
    setProject(info)
    invalidateProjectQueries(qc)
  } catch (e) {
    alert(`Could not open project: ${(e as Error).message}`)
  }
}

async function handleSaveAs(
  qc: ReturnType<typeof useQueryClient>,
  setProject: (p: NonNullable<ReturnType<typeof useStore.getState>['project']>) => void,
) {
  const cur = useStore.getState().project
  if (!cur) return
  try {
    const picked = await save({
      title: `Save ${PRODUCT_NAME} project as`,
      defaultPath: cur.name + '.magpie',
      filters: [{ name: `${PRODUCT_NAME} project`, extensions: ['magpie'] }],
    })
    if (!picked) return
    const info = await saveProjectAs(picked)
    setProject(info)
    invalidateProjectQueries(qc)
  } catch (e) {
    alert(`Could not save project: ${(e as Error).message}`)
  }
}

async function handleClose(
  qc: ReturnType<typeof useQueryClient>,
  setProject: (p: null) => void,
) {
  try {
    await closeProject()
    setProject(null)
    invalidateProjectQueries(qc)
  } catch (e) {
    alert(`Could not close project: ${(e as Error).message}`)
  }
}

function invalidateProjectQueries(qc: ReturnType<typeof useQueryClient>) {
  qc.invalidateQueries({ queryKey: ['current-project'] })
  qc.invalidateQueries({ queryKey: ['app-settings'] })
  qc.invalidateQueries({ queryKey: ['folders'] })
  qc.invalidateQueries({ queryKey: ['images'] })
  qc.invalidateQueries({ queryKey: ['tags'] })
}

function handleOpenMagnifierFromMenu() {
  const state = useStore.getState()
  const sel = state.selection.primary
  if (sel === null) return
  const filter = filterFromView(
    state.view,
    state.extraFilter,
    state.search,
    state.selectedTags,
  )
  void openMagnifierWindow(sel, filter, state.sort).catch(() => {})
}

// ---------------------------------------------------------------------
//                             Undo/Redo
// ---------------------------------------------------------------------

async function handleUndoOrRedo(
  which: 'undo' | 'redo',
  qc: ReturnType<typeof useQueryClient>,
) {
  const store = useStore.getState()
  const entry = which === 'undo' ? store.popUndo() : store.popRedo()
  if (!entry) return
  try {
    const inverse = await applyUndoEntry(entry, which)
    if (which === 'undo') store.pushRedo(inverse)
    else store.pushUndo(inverse)
    qc.invalidateQueries({ queryKey: ['image', entry.id] })
    qc.invalidateQueries({ queryKey: ['images'] })
    qc.invalidateQueries({ queryKey: ['tags'] })
  } catch (e) {
    alert(`Could not ${which}: ${(e as Error).message}`)
    // Put the entry back so the user can try again.
    if (which === 'undo') store.pushUndo(entry)
    else store.pushRedo(entry)
  }
}

async function applyUndoEntry(
  entry: UndoEntry,
  direction: 'undo' | 'redo',
): Promise<UndoEntry> {
  // Undo restores the *from* state; redo re-applies the *to* state.
  // We keep the from/to values on the entry so the same shape works
  // for both stacks — the caller just moves the entry between them.
  switch (entry.kind) {
    case 'title': {
      const target = direction === 'undo' ? entry.from : entry.to
      await updateImageMetadata(entry.id, { title: target })
      return entry
    }
    case 'tags': {
      const target = direction === 'undo' ? entry.from : entry.to
      await batchUpdateMetadata([entry.id], { tags: target })
      return entry
    }
    case 'rename': {
      const target = direction === 'undo' ? entry.from : entry.to
      await renameImage(entry.id, target)
      return entry
    }
  }
}
