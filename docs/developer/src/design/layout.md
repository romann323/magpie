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
│     ├─ TopBar.tsx                    # Top-of-window: add folder, rescan, search, sort
│     ├─ Sidebar.tsx                   # Left: library / tag filters
│     ├─ ImageGrid.tsx                 # Virtualised file grid
│     ├─ DetailsPanel.tsx              # Right: SingleDetails / MultiDetails (Title / Tags / Format / Info)
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
│     ├─ error.rs                      # AppError, AppResult
│     ├─ types.rs                      # Rust-side IPC types (mirror src/types.ts)
│     ├─ db/
│     │  ├─ mod.rs                     # pack_global_id / unpack_global_id helpers
│     │  ├─ registry.rs                # Central registry.db schema + queries
│     │  ├─ library.rs                 # Per-folder library.db schema + queries
│     │  ├─ pool.rs                    # LibraryPool: registry + ATTACHed libraries
│     │  ├─ search.rs                  # Cross-folder query builders (UNION ALL)
│     │  └─ legacy_migration.rs        # One-shot import of pre-redesign central DB
│     ├─ core/
│     │  ├─ mod.rs                     # AppServices, FormatRegistry, dirs
│     │  ├─ scanner.rs                 # Parallel folder scan (writes rel_paths)
│     │  ├─ thumbnail.rs               # Thumb generation + caching (keyed by gid)
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
│     │     ├─ read.rs                 # Delegates to registry + Shell + sidecar
│     │     └─ sidecar.rs              # Legacy `.xmp` reader (never writes)
│     └─ commands/                     # Tauri command handlers
│        ├─ mod.rs
│        ├─ library.rs                 # add/remove/list folders, rescan
│        ├─ images.rs                  # query, get, update, batch_update, delete
│        ├─ tags.rs                    # list / rename / delete tags
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
