import { create } from 'zustand'
import type {
  AppSettings,
  FontSize,
  ImageFilter,
  ImageSort,
  ProjectInfo,
  Theme,
} from './types'

type View =
  | { kind: 'all' }
  | { kind: 'folder'; id: number }
  | { kind: 'smart'; id: number }

type Selection = {
  ids: Set<number>
  primary: number | null
}

/**
 * Session-only undo/redo entries. Each records the *inverse* action so
 * `undo` can just apply it. Redo is populated by every `undo` call.
 */
export type UndoEntry =
  | {
      kind: 'title'
      id: number
      from: string | null
      to: string | null
    }
  | {
      kind: 'tags'
      id: number
      from: string[]
      to: string[]
    }
  | {
      kind: 'rename'
      id: number
      from: string
      to: string
    }

interface Store {
  // -------- Project state ------------------------------------------------
  project: ProjectInfo | null
  setProject: (p: ProjectInfo | null) => void

  settings: AppSettings | null
  setSettings: (s: AppSettings) => void
  setTheme: (t: Theme) => void
  setFontSize: (f: FontSize) => void

  // -------- View / filter ------------------------------------------------
  view: View
  setView: (v: View) => void

  search: string
  setSearch: (s: string) => void

  sort: ImageSort
  setSort: (s: ImageSort) => void

  extraFilter: ImageFilter
  setExtraFilter: (f: ImageFilter) => void

  selectedTags: string[]
  toggleSelectedTag: (name: string) => void
  addSelectedTag: (name: string) => void
  removeSelectedTag: (name: string) => void
  setSelectedTags: (names: string[]) => void
  clearSelectedTags: () => void

  // -------- Grid selection ----------------------------------------------
  selection: Selection
  select: (id: number, mode?: 'replace' | 'add' | 'toggle' | 'range') => void
  clearSelection: () => void

  detailsOpen: boolean
  setDetailsOpen: (open: boolean) => void

  // -------- Undo / redo -------------------------------------------------
  undoStack: UndoEntry[]
  redoStack: UndoEntry[]
  pushUndo: (e: UndoEntry) => void
  popUndo: () => UndoEntry | null
  popRedo: () => UndoEntry | null
  pushRedo: (e: UndoEntry) => void
  clearHistory: () => void
}

export const useStore = create<Store>((set, get) => ({
  project: null,
  setProject: (p) =>
    set({
      project: p,
      // Reset session-scoped state whenever the project changes.
      view: { kind: 'all' },
      selection: { ids: new Set<number>(), primary: null },
      selectedTags: [],
      search: '',
      undoStack: [],
      redoStack: [],
    }),

  settings: null,
  setSettings: (s) => set({ settings: s }),
  setTheme: (t) => set((s) => (s.settings ? { settings: { ...s.settings, theme: t } } : {})),
  setFontSize: (f) =>
    set((s) => (s.settings ? { settings: { ...s.settings, fontSize: f } } : {})),

  view: { kind: 'all' },
  setView: (v) => set({ view: v, selection: { ids: new Set(), primary: null } }),

  search: '',
  setSearch: (s) => set({ search: s }),

  sort: { by: 'takenAt', dir: 'desc' },
  setSort: (s) => set({ sort: s }),

  extraFilter: {},
  setExtraFilter: (f) => set({ extraFilter: f }),

  selectedTags: [],
  toggleSelectedTag: (name) => {
    const cur = get().selectedTags
    const has = cur.some((t) => t.toLowerCase() === name.toLowerCase())
    if (has) {
      set({ selectedTags: cur.filter((t) => t.toLowerCase() !== name.toLowerCase()) })
    } else {
      set({ selectedTags: [...cur, name] })
    }
  },
  addSelectedTag: (name) => {
    const cur = get().selectedTags
    if (cur.some((t) => t.toLowerCase() === name.toLowerCase())) return
    set({ selectedTags: [...cur, name] })
  },
  removeSelectedTag: (name) =>
    set({
      selectedTags: get().selectedTags.filter(
        (t) => t.toLowerCase() !== name.toLowerCase(),
      ),
    }),
  setSelectedTags: (names) => set({ selectedTags: [...names] }),
  clearSelectedTags: () => set({ selectedTags: [] }),

  selection: { ids: new Set<number>(), primary: null },
  select: (id, mode = 'replace') => {
    const cur = get().selection
    if (mode === 'replace') {
      set({ selection: { ids: new Set([id]), primary: id } })
    } else if (mode === 'add') {
      const ids = new Set(cur.ids)
      ids.add(id)
      set({ selection: { ids, primary: id } })
    } else if (mode === 'toggle') {
      const ids = new Set(cur.ids)
      if (ids.has(id)) ids.delete(id)
      else ids.add(id)
      set({ selection: { ids, primary: ids.size > 0 ? id : null } })
    }
  },
  clearSelection: () => set({ selection: { ids: new Set(), primary: null } }),

  detailsOpen: true,
  setDetailsOpen: (open) => set({ detailsOpen: open }),

  undoStack: [],
  redoStack: [],
  pushUndo: (e) => set((s) => ({ undoStack: [...s.undoStack, e], redoStack: [] })),
  popUndo: () => {
    const stack = get().undoStack
    if (stack.length === 0) return null
    const top = stack[stack.length - 1]!
    set({ undoStack: stack.slice(0, -1) })
    return top
  },
  pushRedo: (e) => set((s) => ({ redoStack: [...s.redoStack, e] })),
  popRedo: () => {
    const stack = get().redoStack
    if (stack.length === 0) return null
    const top = stack[stack.length - 1]!
    set({ redoStack: stack.slice(0, -1) })
    return top
  },
  clearHistory: () => set({ undoStack: [], redoStack: [] }),
}))

/**
 * Turn a `View` + orthogonal filters into the `ImageFilter` sent to
 * the backend. Selected sidebar tags AND together via `tagsAll`.
 */
export function filterFromView(
  v: View,
  extra: ImageFilter,
  search: string,
  selectedTags: string[],
): ImageFilter {
  const f: ImageFilter = { ...extra }
  const trimmed = search.trim()
  if (trimmed) f.fts = trimmed
  if (selectedTags.length > 0) {
    f.tagsAll = [...(f.tagsAll ?? []), ...selectedTags]
  }
  switch (v.kind) {
    case 'all':
      return f
    case 'folder':
      f.folderIds = [v.id]
      return f
    case 'smart':
      return f
  }
}
