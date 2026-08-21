# Frontend modules

The frontend is intentionally small — the goal is to be a
translation layer between UI events and Tauri commands, not to hold
domain logic.

## `src/main.tsx`

Boots React. Wraps `<App />` in a `QueryClientProvider` with a
single QueryClient configured for:

- `staleTime: 0` — refetch on refocus.
- `retry: 1` — one retry on network / IPC errors.
- Query cache keys convention (below).

## `src/App.tsx`

Owns the top-level layout:

```
┌────────────────────────────────────────────────────┐
│ TopBar                                             │
├────────┬─────────────────────────────┬─────────────┤
│Sidebar │        ImageGrid            │DetailsPanel │
│        │                             │             │
│        │                             │             │
├────────┴─────────────────────────────┴─────────────┤
│ StatusBar                                          │
└────────────────────────────────────────────────────┘
```

Also hosts the global keyboard listener (Delete key), and mounts
event listeners for `app://image-updated` and
`app://images-deleted` — both of which invalidate the relevant
query keys.

## `src/store.ts` (Zustand)

Global UI-only state:

```ts
interface Store {
  view: View                // 'all' | { folderId } | { tag }
  search: string
  sort: ImageSort
  extraFilter: ImageFilter  // sidebar filters composed
  selection: { ids: Set<number>; anchor: number | null }
  detailsOpen: boolean
  // setters
  setView, setSearch, setSort, setSelection, clearSelection, …
}
```

No persistence in v1 — reloading the app resets to `view: 'all'`,
empty search, no selection.

`filterFromView(view, extra, search)` composes the sidebar view, the
extra filter, and the search string into a single `ImageFilter`
sent to `query_images`.

## `src/ipc.ts`

Thin, typed wrappers around `invoke`:

```ts
export const updateImageMetadata = (id: number, patch: MetadataPatch) =>
  invoke<ImageDetails>('update_image_metadata', { id, patch })

export const batchUpdateMetadata = (ids: number[], patch: MetadataPatch) =>
  invoke<number[]>('batch_update_metadata', { ids, patch })

// …
```

Plus event-listener helpers:

```ts
export const onImageUpdated = (h: (id: number) => void) =>
  listen<number>('app://image-updated', e => h(e.payload))
```

And the diagnostic helper `logFrontend(level, msg)` for pushing
crumbs into the backend log.

## `src/types.ts`

Every IPC struct duplicated from Rust, kept in sync manually. See
[IPC boundary](../architecture/ipc.md#type-mirroring) for the
rationale (no derived types).

## `src/features/`

### `TopBar.tsx`

- Add folder button (opens Tauri dialog, calls `add_library_folder`,
  invalidates `['folders']` and `['images']`).
- Rescan button (calls `rescan_all`).
- Live search input (debounced 200 ms → `useStore.setSearch`).
- Sort dropdown + direction toggle.
- Toggle-details icon.

### `Sidebar.tsx`

- `['folders']`, `['tags']` queries fed to three collapsible
  sections.
- Click handlers set `useStore.view` and clear other filters.
- Right-click context menu for `Remove folder`, `Rescan folder`,
  `Rename tag`, `Delete tag`.

### `ImageGrid.tsx`

- `useQuery(['images', filter, sort, page])` returns paginated
  results.
- `useVirtualizer` from TanStack Virtual computes visible rows.
- Each tile is a `Thumbnail` component; `Ctrl / Shift / plain`
  click logic mutates `useStore.selection`.

### `DetailsPanel.tsx`

- Reads `useStore.selection` and dispatches:
  - 0 selected → placeholder.
  - 1 selected → `<SingleDetails id=… />`.
  - >1 selected → `<MultiDetails ids=… />`.

`SingleDetails` renders four fixed sections:

1. **Title** — editable input, auto-saves via `updateImageMetadata`.
2. **Tags** — `TagInput`, auto-saves via `updateImageMetadata`.
3. **Format metadata** — read-only: handler name +
   `canWriteTags` note. Room to grow into per-format editable
   fields (GPS, description, …) as those handlers add them.
4. **File info** — `<dl>` of the `technical` list returned by the
   backend.

Local edit state is seeded from the query result only when the
`id` changes (guarded by `lastLoadedId.current`), avoiding the
"stomp typing on refetch" bug.

`MultiDetails` shows two `TagInput`s (Add / Remove) and an
Apply button. It uses refs (`tagsAddRef`, `tagsRemoveRef`)
mirroring state so the mutation dispatch always sees the latest
values, even if a blur-triggered `setTagsAdd` and a click on
`Apply` land in the same React batch.

### `TagInput.tsx`

- Controlled by parent's `tags: string[]` + `onChange(next: string[])`.
- Commits the current draft on Enter, Space, Comma, or blur.
- Autocompletion pulled from `list_tags(prefix)` via TanStack Query.
- Backspace on empty input pops the last tag.

### `Thumbnail.tsx`

- Resolves `getThumbPath(id, 'small')` on mount, sets `<img src>` to
  the returned asset URL.
- Placeholder while the thumbnail is being generated (rare, only on
  first scan).

### `StatusBar.tsx`

- Listens to `app://scan` events, shows a live progress bar and
  message.
- Static labels for app version, DB size, thumbnail cache size.

## Query key conventions

| Query key                                    | Fetched by                     | Invalidated by                                    |
| -------------------------------------------- | ------------------------------ | ------------------------------------------------- |
| `['folders']`                                | `listLibraryFolders`           | add/remove/rescan folder, delete images           |
| `['tags']`                                   | `listTags`                     | any metadata update, tag rename/delete            |
| `['tag', prefix]`                            | `listTags(prefix)`             | any metadata update                               |
| `['images', filter, sort, page]`             | `queryImages`                  | any image mutation                                |
| `['image', id]`                              | `getImage`                     | `update_image_metadata` (via `setQueryData`),     |
|                                              |                                | `batch_update_metadata` (via `invalidateQueries`),|
|                                              |                                | `app://image-updated` event                    |
| `['smartCollections']`                       | `listSmartCollections`         | create/delete collection                          |

## Styling

- Tailwind for utility classes.
- Global styles in `src/index.css`: dark background, rounded
  buttons, focus rings.
- No CSS-in-JS. Component-scoped styles are className concatenations
  via `clsx` when needed.
