# Repository layout

```
PicOrg/
├─ src/                                # React frontend (TypeScript)
│  ├─ App.tsx                          # Root component; layout + event listeners
│  ├─ main.tsx                         # ReactDOM entry
│  ├─ index.css                        # Tailwind directives + global styles
│  ├─ ipc.ts                           # Typed Tauri command wrappers
│  ├─ store.ts                         # Zustand global UI state
│  ├─ types.ts                         # Mirrors Rust types
│  └─ features/
│     ├─ TopBar.tsx                    # Top-of-window: add folder, rescan, search, sort
│     ├─ Sidebar.tsx                   # Left: library / rating / tag filters
│     ├─ ImageGrid.tsx                 # Virtualised photo grid
│     ├─ DetailsPanel.tsx              # Right: SingleDetails / MultiDetails
│     ├─ StarRating.tsx                # 5-star clickable widget
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
│     ├─ error.rs                      # PicOrgError, PicOrgResult
│     ├─ types.rs                      # Rust-side IPC types (mirror src/types.ts)
│     ├─ db/
│     │  ├─ mod.rs                     # Db (Mutex<Connection>) + with_conn helper
│     │  ├─ migrations.rs              # SQL migrations (schema + FTS fix)
│     │  └─ queries.rs                 # apply_metadata_patch, get_image, etc.
│     ├─ core/
│     │  ├─ mod.rs                     # AppServices, image-ext filter, dirs
│     │  ├─ scanner.rs                 # Parallel folder scan
│     │  ├─ thumbnail.rs               # Thumb generation + caching
│     │  └─ metadata/
│     │     ├─ mod.rs
│     │     ├─ read.rs                 # EXIF + XMP read, sidecar merge
│     │     ├─ write.rs                # Sidecar + embedded XMP write
│     │     ├─ sidecar.rs              # Path helpers (foo.jpg → foo.xmp)
│     │     └─ xmp.rs                  # XMP parse + build + embed in JPEG
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
