# Implementation instructions — automatic AI tagging

Copy this document (or link to it) when implementing automatic AI tagging in Magpie / PicOrg.

## Project context

**Magpie** (repo: PicOrg) is a Tauri 2 desktop photo library app.

| Layer | Stack |
|-------|--------|
| Backend | Rust — `src-tauri/src/` |
| Frontend | React 19 + TypeScript + TanStack Query + Zustand — `src/` |
| IPC | `src/ipc.ts` → Tauri `invoke` |
| Events | Backend emits Tauri events; frontend `listen`s |
| Tags | Stored in SQLite `magpie.db` only (not written to source files) |
| Tag writes | `batch_update_metadata(ids, { tagsAdd: [...] })` |

**Do not add a TopBar button.** AI tagging is automatic and optional.

---

## Revised requirements

1. **Automatic trigger:** When a library folder is added to a project, run AI tag assignment for that folder **after** the normal filesystem scan finishes.
2. **User control:** Add a **Settings → Auto-tag photos** menu item that toggles the feature on/off. Persist the preference in app settings.
3. **Non-blocking:** AI work runs in a background task (same pattern as `scanner::scan_folder`). The UI must remain fully interactive.
4. **Progress UI:** Show AI progress in the **bottom status bar** (`StatusBar.tsx`), mirroring the existing filesystem scan progress bar.

---

## Architecture

```
add_library_folder()
  └─ spawn: scanner::scan_folder()     → app://scan
       └─ on success + aiAutoTag enabled:
            spawn: auto_tag::tag_folder() → app://auto-tag

Settings menu "Auto-tag photos" (checkable)
  └─ toggle aiAutoTag in app-settings.json
```

**Concurrency rules:**

- Filesystem scan and AI tagging must not block each other on the UI thread.
- Chain AI **after** scan completes so image rows and thumbnails exist.
- Use a semaphore inside `auto_tag` (like scanner) so inference does not saturate CPU/GPU.
- If the user adds several folders quickly, queue AI jobs per folder (FIFO) rather than running all in parallel.

---

## Backend tasks

### 1. App settings — add `aiAutoTag`

**Files:** `src-tauri/src/core/project.rs`, `src-tauri/src/commands/settings.rs`, `src/types.ts`

Add to `AppSettings`:

```rust
#[serde(default = "default_ai_auto_tag")]
pub ai_auto_tag: bool,
```

Default: **`false`** (opt-in; first enable via menu should be enough — no separate consent dialog required for this slice unless product asks for one).

Extend `AppSettingsPatch` and `update_app_settings` to accept `ai_auto_tag: Option<bool>`.

Mirror in TypeScript `AppSettings` / `AppSettingsPatch`.

### 2. New module — `src-tauri/src/core/auto_tag/mod.rs`

Create `auto_tag::tag_folder()` with the same structural pattern as `scanner::scan_folder`:

```rust
pub const AUTO_TAG_EVENT: &str = "app://auto-tag";

pub async fn tag_folder(
    services: Arc<AppServices>,
    app_handle: AppHandle,
    folder_id: i64,
) -> AppResult<AutoTagResult>
```

**Per-image loop:**

1. Query image IDs in folder (`queries::…`, image kind only, not missing).
2. Skip if already AI-tagged and `content_hash` unchanged (add DB columns — see §5).
3. Load thumbnail bytes via existing thumbnail cache (`thumbnail::ensure_thumbnails` if needed).
4. Run classifier in `spawn_blocking` (stub OK for first PR — return 2–3 deterministic tags from a fixed vocabulary).
5. Filter by `min_confidence`, cap at `max_tags_per_image`.
6. `apply_metadata_patch({ tags_add })` in a transaction.
7. Update `ai_tagged_at` / `ai_tag_hash` on the image row.
8. Emit progress every image (or every 5 images if performance requires it).

**Progress payload** — add to `src-tauri/src/types.rs`:

```rust
pub struct AutoTagProgress {
    pub folder_id: i64,
    pub processed: i64,
    pub total: i64,
    pub current_path: Option<String>,
    pub tags_added: i64,      // cumulative tags written this run
    pub finished: bool,
}
```

### 3. Hook into folder add

**File:** `src-tauri/src/commands/library.rs` — `add_library_folder`

Replace the bare scan spawn with a chained task:

```rust
tauri::async_runtime::spawn(async move {
    match scanner::scan_folder(...).await {
        Ok(_) => {
            if services_bg.get_settings()?.ai_auto_tag {
                if let Err(e) = auto_tag::tag_folder(services_bg, app_handle_bg, folder_id).await {
                    tracing::error!(error = %e, "auto-tag failed");
                }
            }
        }
        Err(e) => tracing::error!(error = %e, "scan failed"),
    }
});
```

**Do not** trigger AI on `rescan_folder` / `rescan_all` in this slice unless explicitly requested.

### 4. Optional: AI job queue

If multiple folders are added in quick succession, use a simple mutex-protected queue in `AppServices` or a dedicated `AutoTagScheduler` so only one folder is AI-tagged at a time (scan can still run in parallel per folder).

### 5. Database migration

**File:** new migration in `src-tauri/src/db/migrations/` (follow existing pattern)

```sql
ALTER TABLE images ADD COLUMN ai_tagged_at INTEGER;
ALTER TABLE images ADD COLUMN ai_tag_hash TEXT;
```

Update `docs/developer/src/design/schema.md`.

### 6. Classifier stub (Phase 1)

**File:** `src-tauri/src/core/auto_tag/classifier.rs`

For the first PR, implement a **mock classifier** that returns tags from a fixed vocabulary based on image id hash. Structure the trait so a real ONNX/CLIP sidecar can replace it later:

```rust
pub trait ImageClassifier: Send + Sync {
    fn classify(&self, image_bytes: &[u8]) -> AppResult<Vec<TagSuggestion>>;
}
```

No bundled ML model required in this slice.

### 7. Register commands / modules

- Add `pub mod auto_tag;` in `src-tauri/src/core/mod.rs`
- No new frontend-invokable command is strictly required if AI is fully automatic; settings toggle uses existing `update_app_settings`.
- Document any new types in `docs/developer/src/design/commands.md` (events section).

---

## Frontend tasks

### 1. Menu — toggle item

**File:** `src-tauri/src/menu.rs`

Add to **Settings** submenu:

```rust
pub const ID_SETTINGS_AI_AUTO_TAG: &str = "set_ai_auto_tag";
```

Use a **checkable** menu item (`MenuItemBuilder::…` with check state). On build, read current `ai_auto_tag` from settings if available, or default unchecked.

Tauri note: if native checkmarks are awkward at build time, use a plain item whose label reflects state (`Auto-tag photos ✓` / `Auto-tag photos`) and sync on settings load.

**File:** `src/App.tsx` — extend `useMenuRouter`:

```typescript
case 'set_ai_auto_tag':
  return h.onToggleAiAutoTag()
```

Handler:

1. Read current settings from Zustand / React Query cache.
2. Call `updateAppSettings({ aiAutoTag: !current })`.
3. Update Zustand + invalidate `['settings']`.
4. Call new helper `setMenuItemChecked('set_ai_auto_tag', enabled)` if implemented, or relabel via a new Tauri command.

Add optional Tauri command `set_menu_item_checked(id, checked)` alongside existing `set_menu_item_enabled` in `menu.rs`.

Sync menu check state when app loads settings (in `App.tsx` after `getAppSettings` resolves).

### 2. IPC — auto-tag progress event

**File:** `src/ipc.ts`

```typescript
export const onAutoTagProgress = (handler: (p: AutoTagProgress) => void) =>
  listen<AutoTagProgress>('app://auto-tag', (e) => handler(e.payload))
```

Add `AutoTagProgress` to `src/types.ts`.

### 3. Status bar — dual progress display

**File:** `src/features/StatusBar.tsx`

Currently listens only to `app://scan`. Extend to also listen to `app://auto-tag`.

**Display rules:**

| State | Status bar shows |
|-------|------------------|
| Scan running | Existing: `Scanning — N / M` + progress bar |
| AI running | `Auto-tagging — N / M` + progress bar + optional current path |
| Both running | Show **both** lines stacked, or combine: `Scanning… · Auto-tagging…` with two thin bars |
| Finished | `Auto-tag complete` (auto-hide after 2.5 s, same as scan) |

Reuse existing Tailwind progress bar markup from scan.

On `finished: true` for auto-tag, invalidate React Query keys: `['images']`, `['tags']`.

### 4. Remove / do not add TopBar button

Do **not** add an Auto-tag button to `TopBar.tsx`.

### 5. Settings dialog (optional)

If `SettingsDialogs.tsx` exists for theme/font, optionally add a checkbox there too — menu toggle is sufficient for this slice.

---

## Documentation (required by repo rules)

Update in the same PR:

| Doc | Change |
|-----|--------|
| `docs/user-manual/` | New short section: "Automatic tagging" — explain Settings menu toggle, background behaviour, status bar progress, tags go to library only |
| `docs/developer/src/design/testing.md` | How to test auto-tag with mock classifier |
| `docs/developer/src/design/schema.md` | New columns |
| `docs/developer/src/design/commands.md` | `app://auto-tag` event payload |
| `docs/developer/src/functional/out-of-scope.md` | Move auto-tagging from "out of scope" to "implemented (opt-in, local stub)" or similar |

Run: `npm run docs:build`

Use plain language in the user manual (no jargon like "ONNX", "spawn_blocking").

---

## Testing checklist

**Rust integration test** (`src-tauri/tests/`):

1. Add folder with mock classifier enabled → images receive `tagsAdd` entries.
2. Add folder with `aiAutoTag: false` → no tags added by AI.
3. Re-add / unchanged hash → image skipped on second AI run.

**Manual smoke:**

1. Settings → Auto-tag photos → checkmark appears.
2. Add folder → status bar shows scan, then auto-tag progress.
3. UI remains responsive (scroll grid, open details) during auto-tag.
4. Tags appear in sidebar tag list after completion.

---

## File touch list (summary)

```
src-tauri/src/core/auto_tag/mod.rs          (new)
src-tauri/src/core/auto_tag/classifier.rs   (new, mock)
src-tauri/src/core/mod.rs
src-tauri/src/core/project.rs
src-tauri/src/commands/library.rs
src-tauri/src/commands/settings.rs
src-tauri/src/menu.rs
src-tauri/src/types.rs
src-tauri/src/db/migrations/NNN_ai_tag.sql  (new)
src-tauri/src/db/queries.rs                 (skip logic, column updates)
src/types.ts
src/ipc.ts
src/App.tsx
src/features/StatusBar.tsx
docs/…
```

---

## Explicit non-goals for this slice

- No TopBar button
- No cloud API
- No real ML model bundle (mock classifier only)
- No AI on manual rescan
- No writing tags into source files
- No cancel button (can be a follow-up)

---

## Suggested PR title

`feat: opt-in automatic AI tagging after folder add with status bar progress`

---

## Reference implementation patterns

Read these files first, then mirror their patterns:

- `src-tauri/src/core/scanner.rs` — background job, semaphore, progress events
- `src-tauri/src/commands/library.rs` — `add_library_folder` spawn hook
- `src/features/StatusBar.tsx` — scan progress bar UI
- `src/App.tsx` — `useMenuRouter` for Settings menu actions
- `src-tauri/src/menu.rs` — native menu item IDs and build
