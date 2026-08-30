# Repository layout

```
Magpie/
├─ src/                                # React frontend (TypeScript)
│  ├─ App.tsx                          # Root component; layout + event listeners
│  ├─ main.tsx                         # ReactDOM entry
│  ├─ index.css                        # Tailwind directives + global styles
│  ├─ ipc.ts                           # Typed Tauri command wrappers
│  ├─ store.ts                         # Zustand global UI state
│  ├─ types.ts                         # Mirrors Rust types
│  └─ features/
│     ├─ TopBar.tsx                    # Top-of-window: add folder, rescan, SearchBox, sort
│     ├─ Sidebar.tsx                   # Left: library / folder / tag multi-select
│     ├─ SearchBox.tsx                 # Chips (tags) + free-text FTS input
│     ├─ ImageGrid.tsx                 # Virtualised file grid (double-click → Magnifier window)
│     ├─ DetailsPanel.tsx              # Right: SingleDetails / MultiDetails (Title / Tags / Format / Info + editable filename)
│     ├─ MagnifierWindow.tsx           # Root component of the separate Magnifier pop-up window
│     ├─ openMagnifierWindow.ts        # Helper that spawns/focuses that window
│     ├─ WelcomeScreen.tsx             # Shown when no project is open
│     ├─ SettingsDialogs.tsx           # Theme / Font-size / Language modals
│     ├─ TagInput.tsx                  # Tag entry with autocompletion
│     ├─ Thumbnail.tsx                 # <img> wrapper with async src
│     └─ StatusBar.tsx                 # Bottom: scan progress, app version
│
├─ src-tauri/                          # Rust backend (Cargo package)
│  ├─ Cargo.toml
│  ├─ tauri.conf.json                  # App config: identifier, security, window
│  ├─ build.rs
│  ├─ capabilities/
│  │  └─ default.json                  # Capability manifest (asset scopes, etc.)
│  ├─ examples/
│  │  ├─ dump_meta.rs                  # Diagnostic: dump everything about a photo
│  │  └─ dump_tag_usage.rs             # Diagnostic: find photos tagged X
│  ├─ tests/
│  │  └─ metadata_fs.rs                # Integration tests for metadata pipeline
│  └─ src/
│     ├─ main.rs                       # Windows subsystem entry — calls lib::run
│     ├─ lib.rs                        # Tauri builder, logging, handler registration
│     ├─ menu.rs                       # Native menu bar + menu-event bridge
│     ├─ error.rs                      # AppError, AppResult
│     ├─ types.rs                      # Rust-side IPC types (mirror src/types.ts)
│     ├─ db/
│     │  ├─ mod.rs                     # Db handle (Arc<Mutex<Connection>>)
│     │  ├─ schema.rs                  # Apply schema.sql on fresh DBs
│     │  ├─ schema.sql                 # DDL for a project DB
│     │  ├─ queries.rs                 # All SQL against the project DB
│     │  └─ migrate.rs                 # One-shot import from legacy DB layouts
│     ├─ core/
│     │  ├─ mod.rs                     # AppServices (holds ProjectState + AppSettings)
│     │  ├─ project.rs                 # Projects + AppSettings persistence
│     │  ├─ scanner.rs                 # Parallel folder scan (writes rel_paths)
│     │  ├─ thumbnail.rs               # Thumb generation + caching (keyed by image_id)
│     │  ├─ formats/                   # Per-format handlers, all read-only
│     │  │  ├─ mod.rs                  # FormatHandler trait + FormatRegistry
│     │  │  ├─ common.rs               # EXIF/dims utilities, verbatim-path helpers
│     │  │  ├─ xmp_packet.rs           # XMP parser (read only)
│     │  │  ├─ win_shell.rs            # Windows IPropertyStore reader
│     │  │  ├─ jpeg.rs
│     │  │  ├─ png.rs
│     │  │  ├─ webp.rs
│     │  │  ├─ gif.rs
│     │  │  ├─ tiff.rs
│     │  │  └─ stubs.rs                # HEIC, PDF, video, RAW, …
│     │  └─ metadata/
│     │     ├─ mod.rs
│     │     ├─ read.rs                 # Reads at scan time; populates magpie.db
│     │     └─ sidecar.rs              # Legacy `.xmp` reader (never writes)
│     └─ commands/                     # Tauri command handlers
│        ├─ mod.rs
│        ├─ project.rs                 # current/create/open/save[_as]/close
│        ├─ settings.rs                # get/update app settings JSON
│        ├─ library.rs                 # add/remove/list folders, rescan
│        ├─ images.rs                  # query, get, update, batch_update, delete, rename
│        ├─ tags.rs                    # list / rename / delete tags
│        ├─ magnifier.rs               # Magnifier window context (get/set)
│        ├─ collections.rs             # smart collections (skeleton in v1)
│        ├─ thumbs.rs                  # thumb + asset path resolvers
│        └─ diag.rs                    # log_frontend bridge
│
├─ scripts/
│  ├─ screenshot-app.ps1               # Verify UI screenshot
│  ├─ screenshot-scrolled.ps1          # Screenshot with programmatic scroll
│  └─ test-multiselect-tag.ps1         # E2E UI test skeleton
│
├─ docs/                               # This documentation
│  ├─ user-manual/                     # mdBook: user-facing manual
│  ├─ developer/                       # mdBook: this guide
│  ├─ shared/                          # Shared CSS / JS for both books
│  ├─ index.html                       # Landing page linking both docs
│  └─ build.ps1                        # HTML + PDF build script
│
├─ index.html                          # Vite entry
├─ vite.config.ts
├─ tailwind.config.js
├─ postcss.config.js
├─ tsconfig.json
├─ package.json
├─ README.md
└─ .gitignore
```

## What lives where

- **Anything user-visible** (button text, layout, animations) is
  under `src/`.
- **Anything touching disk** (files, DB) is under `src-tauri/src/`.
  If you find frontend code doing FS work, that's a bug.
- **Anything that runs at build time** (Tauri config, capabilities,
  Cargo settings) is under `src-tauri/` outside `src/`.
- **Documentation source** is under `docs/`. Generated HTML lands
  in `docs/user-manual/book/` and `docs/developer/book/`; PDFs land
  in `docs/`.
