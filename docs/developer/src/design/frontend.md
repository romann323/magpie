# Frontend modules

The frontend is intentionally small — the goal is to be a
translation layer between UI events and Tauri commands, not to hold
domain logic.

## `src/main.tsx`

Boots React. Wraps the rendered component in a `QueryClientProvider`
with a single QueryClient configured for:

- `staleTime: 0` — refetch on refocus.
- `retry: 1` — one retry on network / IPC errors.
- Query cache keys convention (below).

`main.tsx` also acts as a tiny router: if `location.hash` is
`#magnifier` we render `<MagnifierWindow />` (see below), otherwise
`<App />`. This lets both windows share one Vite bundle instead of
requiring a second HTML entry point.

## `src/App.tsx`

Owns the top-level layout, project bootstrap, and menu routing.

The main-UI wrapper `<div>` is `key`ed on `project.path` so switching
projects fully unmounts the tree — this resets React Query state,
virtualised grid scroll, and the `Thumbnail` `<img src>` caches so
nothing from the previous project leaks through when the new one
loads.


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

Project bootstrap:

- Fetches `current_project` and `get_app_settings` on mount via
  React Query, then seeds `useStore` accordingly.
- If `current_project` returns `null`, renders `<WelcomeScreen />`
  instead of the layout above.
- Applies `settings.theme` and `settings.fontSize` to the `<html>`
  element (adds `theme-*` and `font-size-*` classes).

Menu routing:

- `useMenuRouter({ … })` subscribes to `app://menu` and dispatches
  each menu ID to a handler (`handleNewProject`, `handleOpenProject`,
  `handleSaveAs`, `handleClose`, `handleOpenMagnifierFromMenu`,
  `handleUndoOrRedo('undo' | 'redo')`, `setSettingsDialog(…)`).
- Separate `useEffect`s call `setMenuItemEnabled` for
  `edit_undo` / `edit_redo` (driven by stack length) and
  `view_magnifier` (driven by `selection.primary != null`).

Also hosts:

- The global keyboard listener (Delete, Escape).
- `app://image-updated` and `app://images-deleted` listeners that
  invalidate the relevant query keys.
- The `<Magnifier />` and `<SettingsDialogs />` overlay mounts.

## `src/store.ts` (Zustand)

Global UI-only state:

```ts
interface Store {
  // Project + app settings
  project: ProjectInfo | null
  settings: AppSettings | null
  setProject, setSettings, setTheme, setFontSize

  // Browse + search
  view: View                // 'all' | { kind: 'folder', folderId } | 'untagged' | 'missing'
  search: string
  selectedTags: string[]    // AND-combined; drives sidebar tag bubbles + SearchBox chips
  sort: ImageSort
  extraFilter: ImageFilter
  toggleSelectedTag, addSelectedTag, removeSelectedTag,
    setSelectedTags, clearSelectedTags

  // Selection + right pane
  selection: { ids: Set<number>; anchor: number | null }
  detailsOpen: boolean

  // Session-scoped undo/redo
  undoStack: UndoEntry[]
  redoStack: UndoEntry[]
  pushUndo, popUndo, pushRedo, popRedo, clearHistory
}
```

`setProject(new)` clears the entire per-project session
(view, selection, tags, search, undo/redo). This is what keeps the
UI honest when the user switches projects. The magnifier lives in a
separate window and has its own state on the Rust side, so it does
**not** live in this store.

`filterFromView(view, extra, search, selectedTags)` composes the
sidebar view, the extra filter, the free-text search, **and the
selected-tag list** into one `ImageFilter` sent to `query_images`.
Multiple tags AND together server-side.

`UndoEntry`:

```ts
type UndoEntry =
  | { kind: 'title';  id: number; from: string; to: string }
  | { kind: 'tags';   id: number; from: string[]; to: string[] }
  | { kind: 'rename'; id: number; from: string; to: string }
```

Undo/redo is session-scoped (in-memory, per app run). Closing the
project or the app clears it.

## `src/ipc.ts`

Thin, typed wrappers around `invoke`:

```ts
export const updateImageMetadata = (id: number, patch: MetadataPatch) =>
  invoke<ImageDetails>('update_image_metadata', { id, patch })

export const renameImage = (id: number, newFilename: string) =>
  invoke<ImageDetails>('rename_image', { id, newFilename })

export const currentProject = () =>
  invoke<ProjectInfo | null>('current_project')

export const createProject = (path: string) =>
  invoke<ProjectInfo>('create_project', { path })

// …plus openProject, saveProject, saveProjectAs, closeProject,
//    getAppSettings, updateAppSettings, setMenuItemEnabled.
```

Event-listener helpers:

```ts
export const onImageUpdated = (h: (id: number) => void) =>
  listen<number>('app://image-updated', e => h(e.payload))

export const onMenuEvent = (h: (menuId: string) => void) =>
  listen<string>('app://menu', e => h(e.payload))
```

And the diagnostic helper `logFrontend(level, msg)` for pushing
crumbs into the backend log.

## `src/types.ts`

Every IPC struct duplicated from Rust, kept in sync manually. See
[IPC boundary](../architecture/ipc.md#type-mirroring) for the
rationale (no derived types).

Notable additions for the project model:

```ts
export type ProjectInfo = { path: string; name: string }
export type Theme      = 'system' | 'dark' | 'light'
export type FontSize   = 'small' | 'medium' | 'large'
export type AppSettings = {
  theme: Theme
  fontSize: FontSize
  language: string
  lastProjectPath: string | null
  recentProjects: string[]
}
export type AppSettingsPatch =
  Partial<Pick<AppSettings, 'theme' | 'fontSize' | 'language'>>
```

## `src/features/`

### `TopBar.tsx`

- Add-folder button (opens Tauri dialog, calls `add_library_folder`,
  invalidates `['folders']` and `['images']`).
- Rescan button (calls `rescan_all`).
- **`<SearchBox />`** — chips for every selected tag plus free-text
  search input. Not present on the welcome screen.
- Sort dropdown + direction toggle.
- Toggle-details icon.
- Current project name (with full path on hover) at the right edge.

### `SearchBox.tsx`

- Renders one `<Chip>` per entry in `useStore.selectedTags`;
  clicking the chip's `×` calls `removeSelectedTag`, which
  automatically deselects the sidebar tag bubble for that tag.
- Free-text input is debounced (200 ms) into `useStore.setSearch`.
- Enter submits the current draft.

### `Sidebar.tsx`

- `['folders']`, `['tags']` queries fed to collapsible sections.
- Folders/quick-filters set `useStore.view` and clear the selection.
- Tags render as `<TagBubble>` pills inside a `flex flex-wrap`
  container, so they flow across as many rows as they need in the
  sidebar's width. Clicking a bubble calls `toggleSelectedTag`;
  selected bubbles are filled with the accent colour and unselected
  bubbles use the raised-surface background. Each bubble carries a
  small numeric badge with the tag's usage count. A `Clear all`
  button at the top of the Tags section calls `clearSelectedTags`.
- Right-click context menu for `Remove folder`, `Rescan folder`,
  `Rename tag`, `Delete tag`.

### `ImageGrid.tsx`

- `useQuery(['images', filter, sort, page])` returns paginated
  results. `filter` is produced via
  `filterFromView(view, extra, search, selectedTags)`.
- `useVirtualizer` from TanStack Virtual computes visible rows.
- Each tile is a `Thumbnail` component; `Ctrl / Shift / plain`
  click logic mutates `useStore.selection`.
- **Double-click** on a tile calls `useStore.openMagnifier(img.id)`.

### `DetailsPanel.tsx`

- Reads `useStore.selection` and dispatches:
  - 0 selected → placeholder.
  - 1 selected → `<SingleDetails id=… />`.
  - >1 selected → `<MultiDetails ids=… />`.

`SingleDetails` renders the following sections plus an editable
file name:

1. **Title** — editable input, auto-saves via `updateImageMetadata`.
   Pushes an `UndoEntry` for every successful save.
2. **Your tags** — `TagInput`, backed by `ImageDetails.userTags`.
   Auto-saves via `updateImageMetadata` (which targets the `'user'`
   source on the Rust side). Also pushes an `UndoEntry`.
3. **Automatic tags** — read-only `<ReadOnlyTagList>` fed by
   `ImageDetails.autoTags`. **Always rendered**, even when the list
   is empty (an italic "No automatic tags on this file." placeholder
   takes the pill row's place), so the distinction between editable
   and read-only tags stays discoverable on every file. Extra
   affordances make the read-only nature obvious:
   - A `<LockIcon>` next to the section label.
   - Each pill carries its own small lock glyph.
   - The container uses a dashed border, muted colours, and
     `cursor-not-allowed`.
   - `aria-readonly="true"` on the container so screen readers
     announce it correctly.
   The mirror section **Your tags** carries a `<PencilIcon>` in the
   same slot so the two categories are visually paired.
4. **Format metadata** — read-only: handler name. Room to grow
   into per-format editable fields (GPS, description, …) as those
   handlers grow their surface area.
5. **File info** — `<dl>` of the `technical` list returned by the
   backend plus filename, size, format, mtime, and import time.
   The **filename** is inline-editable: Enter calls `renameImage`,
   Escape or blur reverts the input.
6. **Preview** — double-clicking the small preview opens the
   Magnifier for that image.

Local edit state is seeded from the query result only when the
`id` changes (guarded by `lastLoadedId.current`), avoiding the
"stomp typing on refetch" bug.

`MultiDetails` shows two `TagInput`s (Add / Remove) and an
Apply button. It uses refs (`tagsAddRef`, `tagsRemoveRef`)
mirroring state so the mutation dispatch always sees the latest
values, even if a blur-triggered `setTagsAdd` and a click on
`Apply` land in the same React batch.

### `MagnifierWindow.tsx` + `openMagnifierWindow.ts`

The magnifier is a **separate native Tauri window**, not an in-app
modal.

- `openMagnifierWindow(imageId, filter, sort)` (helper in
  `features/openMagnifierWindow.ts`) is called by
  `ImageGrid.onDoubleClick`, `DetailsPanel`'s preview double-click,
  and `App.tsx`'s `View → Magnifier` menu handler. It first calls
  `setMagnifierContext` to stash the DTO on the Rust side, then
  either focuses the existing `WebviewWindow` labelled `"magnifier"`
  (and emits `app://magnifier-reset` to refresh it) or creates a new
  one pointing at `index.html#magnifier`.
- `MagnifierWindow.tsx` is what `main.tsx` mounts for that route.
  It fetches `getMagnifierContext()` on mount, runs the same
  `queryImages(filter, sort, { limit: 5000 })` the grid is using, and
  paints the current image via `<img src={toAssetUrl(cur.path)}>`.
  Prev / Next / Esc all work; the window title mirrors the current
  filename.

Both windows share the same origin (and therefore the same asset
protocol scope, IPC handler, and SQLite handle), so nothing extra is
needed to render pictures from the source folders.

### `WelcomeScreen.tsx`

- Rendered by `App.tsx` when `useStore.project === null`.
- Two primary buttons: **New Project…** (Tauri save dialog) and
  **Open Project…** (Tauri open dialog).
- List of recent projects from `AppSettings.recentProjects`,
  each row invokes `openProject`.
- On success calls `setProject(info)` and lets React Query refetch
  everything for the new project.

### `SettingsDialogs.tsx`

- Modal shell with a small radio-group form.
- `<ThemeDialog>` — System / Dark / Light. Updates
  `settings.theme` via `updateAppSettings`; `App.tsx` reapplies
  the `<html>` class immediately.
- `<FontSizeDialog>` — Small / Medium / Large. Same pattern.
- `<LanguageDialog>` — placeholder in v1 (English only).

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
| `['project']`                                | `currentProject`               | any `project::*` command                          |
| `['settings']`                               | `getAppSettings`               | `updateAppSettings`                               |
| `['folders']`                                | `listLibraryFolders`           | add/remove/rescan folder, delete images           |
| `['tags']`                                   | `listTags`                     | any metadata update, tag rename/delete            |
| `['tag', prefix]`                            | `listTags(prefix)`             | any metadata update                               |
| `['images', filter, sort, page]`             | `queryImages`                  | any image mutation                                |
| `['image', id]`                              | `getImage`                     | `update_image_metadata` (via `setQueryData`),     |
|                                              |                                | `batch_update_metadata` (via `invalidateQueries`),|
|                                              |                                | `rename_image`,                                   |
|                                              |                                | `app://image-updated` event                       |
| `['smartCollections']`                       | `listSmartCollections`         | create/delete collection                          |

## Styling

- Tailwind for utility classes.
- Global styles in `src/index.css`: dark background, rounded
  buttons, focus rings.
- Theme classes (`html.theme-light`, `html.theme-dark`,
  `html.theme-system`) apply the light-mode overrides on top of
  the default dark palette.
- Font-size classes (`html.font-size-small|medium|large`) set the
  root `font-size` so every `rem`-based Tailwind value scales.
- Modal styles (`.modal-backdrop`, `.modal-panel`, `.modal-card`)
  are used by `Magnifier` and `SettingsDialogs`.
- No CSS-in-JS. Component-scoped styles are className concatenations
  via `clsx` when needed.
