import { create } from 'zustand'
import type { ImageFilter, ImageSort } from './types'

type View =
  | { kind: 'all' }
  | { kind: 'folder'; id: number }
  | { kind: 'rating'; min: number }
  | { kind: 'tag'; name: string }
  | { kind: 'smart'; id: number }

type Selection = {
  ids: Set<number>
  primary: number | null
}

interface Store {
  view: View
  setView: (v: View) => void

  search: string
  setSearch: (s: string) => void

  sort: ImageSort
  setSort: (s: ImageSort) => void

  extraFilter: ImageFilter
  setExtraFilter: (f: ImageFilter) => void

  selection: Selection
  select: (id: number, mode?: 'replace' | 'add' | 'toggle' | 'range') => void
  clearSelection: () => void

  detailsOpen: boolean
  setDetailsOpen: (open: boolean) => void
}

export const useStore = create<Store>((set, get) => ({
  view: { kind: 'all' },
  setView: (v) => set({ view: v, selection: { ids: new Set(), primary: null } }),

  search: '',
  setSearch: (s) => set({ search: s }),

  sort: { by: 'takenAt', dir: 'desc' },
  setSort: (s) => set({ sort: s }),

  extraFilter: {},
  setExtraFilter: (f) => set({ extraFilter: f }),

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
}))

export function filterFromView(v: View, extra: ImageFilter, search: string): ImageFilter {
  const f: ImageFilter = { ...extra }
  if (search.trim()) f.fts = search.trim()

  switch (v.kind) {
    case 'all':
      return f
    case 'folder':
      f.folderIds = [v.id]
      return f
    case 'rating':
      f.ratingMin = v.min
      return f
    case 'tag':
      f.tagsAll = [...(f.tagsAll ?? []), v.name]
      return f
    case 'smart':
      // Combined at query time by pulling the smart collection's filter and merging.
      return f
  }
}
