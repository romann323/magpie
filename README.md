# Magpie

Desktop app for organizing photos by metadata. Add folders, scan for images, edit their title / rating / tags / comments, and browse or filter your library by that metadata. All metadata is written back **inside the source image file** as XMP so it's interoperable with Adobe Lightroom, digiKam, Adobe Bridge, and Windows Explorer.

Built with **Tauri 2 + React + TypeScript + Rust** for a small footprint (~13 MB executable, ~3.4 MB installer) and native performance.

## Features (v0.1)

- **Multiple watched folders** — recursively scan any number of folders.
- **Wide format support (read)** — JPEG, PNG, WebP, GIF, BMP, TIFF, HEIC/HEIF, plus common camera RAW formats (CR2, CR3, NEF, ARW, DNG, RAF, ORF, RW2, SRW). EXIF + XMP are extracted where present, and legacy `.xmp` sidecars are picked up for backward compatibility.
- **Metadata editing** — Title, Rating (0–5 stars), Tags, Comments.
- **Portable metadata (write)** — embedded directly inside JPEG (APP1) and PNG (`iTXt`) source files so Windows Explorer, Lightroom, Bridge, and digiKam all see the same tags. Never creates sidecar files; for formats we can't embed into yet (RAW, HEIC, TIFF, WebP, GIF, BMP) the UI surfaces a clear "unsupported format" message.
- **Fast, virtualized grid** — comfortably browses libraries of thousands of images.
- **Filters** — by folder, minimum rating, tag, or free-text search across title/comment/filename/tags.
- **Sorting** — by capture date, filename, rating, size, or added order.
- **Multi-select batch editing** — apply ratings or add/remove tags across many images at once.

## Requirements to build

- Windows 10 or 11 (x64).
- [Rustup](https://www.rust-lang.org/tools/install) with the `stable-x86_64-pc-windows-msvc` toolchain.
- Visual Studio Build Tools 2022 with the "Desktop development with C++" workload.
- Node.js 20+.
- Microsoft Edge WebView2 runtime (pre-installed on Windows 10/11).

The installer bootstraps WebView2 automatically on target machines that don't have it.

## Build

```powershell
# One-time
npm install

# Development (opens the app with hot-reload)
npm run tauri:dev

# Production build - creates dist/ and a Windows installer
npm run tauri:build
```

Build outputs:

- `src-tauri/target/release/desktop.exe` — the standalone executable (~13 MB).
- `src-tauri/target/release/bundle/nsis/Magpie_<version>_x64-setup.exe` — the installer (~3.4 MB).

## Documentation

Full documentation is generated from Markdown into two browsable
[mdBook](https://rust-lang.github.io/mdBook/) sites, each with a downloadable
PDF. Open [`docs/index.html`](docs/index.html) to browse:

- **User Manual** — task-oriented guide (installation, editing metadata,
  multi-select, deletion, keyboard shortcuts, troubleshooting).
- **Developer Guide** — three modules: Functional Overview, Architecture,
  Detailed Design (repo layout, DB schema, Tauri commands, algorithms,
  testing strategy, build & release).

Regenerate after editing anything under `docs/user-manual/src/` or
`docs/developer/src/`:

```powershell
npm run docs:build          # rebuilds HTML + PDFs
npm run docs:serve          # rebuilds and serves on http://localhost:8000
```

The build step requires `mdbook` (installed once via
`cargo install mdbook --version 0.4.40 --locked`) and Microsoft Edge or
Chrome for the PDF rendering.

## Architecture

```
┌─────────────────────────────────────────────┐
│  React + TypeScript UI                      │
│  (grid, sidebar, metadata panel, tag picker)│
└─────────────────┬───────────────────────────┘
                  │ Tauri IPC (typed commands + events)
┌─────────────────▼───────────────────────────┐
│  Rust core                                   │
│  ├─ Folder scanner (jwalk + tokio)           │
│  ├─ Metadata reader (EXIF + embedded XMP     │
│  │   in JPEG APP1 / PNG iTXt; legacy .xmp    │
│  │   sidecars still read for compatibility)  │
│  ├─ Metadata writer (embeds XMP inside the   │
│  │   source file, atomic temp+rename)        │
│  ├─ Thumbnail generator (fast_image_resize)  │
│  └─ SQLite index (rusqlite + FTS5)           │
└─────────────────┬───────────────────────────┘
                  │
┌─────────────────▼───────────────────────────┐
│  Your image folders + %APPDATA%\Magpie\     │
│  ├─ Image files (only bytes ever changed:   │
│  │   the XMP block inside the file itself)  │
│  ├─ library.db (SQLite index)                 │
│  └─ thumbs\ (WebP thumbnail cache)           │
└─────────────────────────────────────────────┘
```

The file on disk is the source of truth. SQLite is a rebuildable cache — delete the DB and everything rebuilds from the files.

## Where your data lives

- **Metadata:** embedded directly inside the source image (JPEG APP1 XMP segment or PNG `iTXt` chunk with the Adobe-standard keyword `XML:com.adobe.xmp`). Uses `dc:title`, `xmp:Rating`, `dc:description`, and `dc:subject` so Lightroom, digiKam, Bridge, and Windows Explorer read the same data. Legacy `.xmp` sidecars left by older Magpie versions or Lightroom are still read on first scan and then cleaned up by the next save.
- **Index + cache:** `%APPDATA%\com.magpie.app\` — SQLite DB and WebP thumbnails.

## Cross-platform

Tauri produces platform-native binaries. Windows is the primary target for v1. To build for macOS in the future:

```powershell
rustup target add aarch64-apple-darwin
cargo build --release --target aarch64-apple-darwin
```

The Rust code is 100% cross-platform; only signing/notarization differs.

## License

MIT
