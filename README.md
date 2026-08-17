# PicOrg

Desktop app for organizing photos by metadata. Add folders, scan for images, edit their title / rating / tags / comments, and browse or filter your library by that metadata. All metadata is written back to disk as XMP so it's interoperable with Adobe Lightroom, digiKam, Adobe Bridge, and other tools.

Built with **Tauri 2 + React + TypeScript + Rust** for a small footprint (~13 MB executable, ~3.4 MB installer) and native performance.

## Features (v0.1)

- **Multiple watched folders** — recursively scan any number of folders.
- **Wide format support** — JPEG, PNG, WebP, GIF, BMP, TIFF (thumbnails + metadata) plus RAW (metadata only).
- **Metadata editing** — Title, Rating (0–5 stars), Tags, Comments.
- **Portable metadata** — written as sidecar `.xmp` files (Lightroom-compatible; the original image files are never modified).
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

- `src-tauri/target/release/picorg.exe` — the standalone executable (~13 MB).
- `src-tauri/target/release/bundle/nsis/PicOrg_<version>_x64-setup.exe` — the installer (~3.4 MB).

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
│  ├─ Metadata reader (EXIF, XMP, sidecar)    │
│  ├─ Metadata writer (XMP sidecar, atomic)   │
│  ├─ Thumbnail generator (fast_image_resize) │
│  └─ SQLite index (rusqlite + FTS5)          │
└─────────────────┬───────────────────────────┘
                  │
┌─────────────────▼───────────────────────────┐
│  Your image folders + %APPDATA%\PicOrg\     │
│  ├─ Image files (untouched)                  │
│  ├─ Image.xmp sidecars (metadata)            │
│  ├─ picorg.db (SQLite index)                 │
│  └─ thumbs\ (WebP thumbnail cache)           │
└─────────────────────────────────────────────┘
```

The file on disk is the source of truth. SQLite is a rebuildable cache — delete the DB and everything rebuilds from the files.

## Where your data lives

- **Metadata:** in an XMP sidecar file (`Photo.xmp`) next to each image. Uses the standard `dc:title`, `xmp:Rating`, `dc:description`, and `dc:subject` fields, so Lightroom, digiKam, Bridge, etc. read/write the same data.
- **Index + cache:** `%APPDATA%\PicOrg\` — SQLite DB and WebP thumbnails.

## Cross-platform

Tauri produces platform-native binaries. Windows is the primary target for v1. To build for macOS in the future:

```powershell
rustup target add aarch64-apple-darwin
cargo build --release --target aarch64-apple-darwin
```

The Rust code is 100% cross-platform; only signing/notarization differs.

## License

MIT
