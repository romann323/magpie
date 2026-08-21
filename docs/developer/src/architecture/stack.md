# Technology stack

Magpie uses a small, deliberate set of libraries. Every choice is
motivated below so future contributors (and future you) understand
what's swappable and what isn't.

## Backend (Rust)

| Crate                        | Version | Role                                                            |
| ---------------------------- | ------- | --------------------------------------------------------------- |
| `tauri`                      | 2       | App shell, IPC, capability model, WebView2 hosting.             |
| `tauri-plugin-dialog`        | 2       | Native folder-picker and confirmation dialogs.                  |
| `tauri-plugin-opener`        | 2       | Opens URLs and paths in the OS default handler.                 |
| `tokio`                      | 1       | Async runtime for all Tauri commands.                           |
| `rusqlite`                   | 0.32    | SQLite bindings; `bundled` feature so we ship SQLite ourselves. |
| `jwalk`                      | 0.8     | Parallel recursive directory traversal for the scanner.         |
| `rayon`                      | 1.10    | Parallel work-stealing for CPU-bound phases (hashing, thumbs).  |
| `image`                      | 0.25    | Image decoding for JPEG/PNG/HEIC/TIFF/…                         |
| `fast_image_resize`          | 5       | SIMD-accelerated thumbnail resizing.                            |
| `kamadak-exif`               | 0.6     | EXIF parsing (taken time, camera make/model, dimensions).       |
| `quick-xml`                  | 0.36    | XMP packet parsing (streaming, no DOM).                         |
| `xxhash-rust`                | 0.8     | Fast content hashing (`XXH3`).                                  |
| `chrono`                     | 0.4     | Timestamps, serialisation.                                      |
| `dirs`                       | 5       | Locate `%APPDATA%` cross-platform.                              |
| `trash`                      | 5       | Cross-platform Recycle Bin move.                                |
| `tracing` + `tracing-subscriber` | 0.1 / 0.3 | Structured logging, file sink.                              |
| `serde` + `serde_json`       | 1       | IPC (de)serialisation of every Tauri argument/return.           |
| `thiserror` + `anyhow`       | 1       | Error boilerplate.                                              |

**Why Rust over Node / Go / C++:**

- Zero-cost async is a great fit for the file-I/O-heavy workload.
- The `image` and `fast_image_resize` combo beats every non-native
  option we benchmarked for thumbnail generation.
- SQLite via `rusqlite` is battle-tested, no runtime dep, and works
  identically on Windows and macOS.
- Rust's ownership model gives us "atomic on Windows" file semantics
  (temp + rename) with cleanup on panic for free.

## Frontend (TypeScript + React)

| Package                                 | Role                                                          |
| --------------------------------------- | ------------------------------------------------------------- |
| `react` / `react-dom` (v18)             | UI framework.                                                 |
| `typescript` (v5)                       | Static types across every file (`.ts` / `.tsx`).              |
| `vite`                                  | Dev server with HMR, production bundler.                      |
| `tailwindcss`                           | Utility-first CSS; near-zero runtime.                         |
| `@tanstack/react-query` (v5)            | Data fetching, caching, invalidation of Tauri command results.|
| `@tanstack/react-virtual` (v3)          | Virtualised grid — draws only visible tiles.                  |
| `zustand`                               | Lightweight global UI state (selection, sort, filters).       |
| `@tauri-apps/api` (v2)                  | `invoke` and `listen` for IPC.                                |
| `@tauri-apps/plugin-dialog`             | Frontend side of `tauri-plugin-dialog`.                       |
| `clsx`                                  | Tiny className concatenation helper.                          |

**Why React over Vue / Svelte / Solid:**

- Ecosystem depth: TanStack Query and React Virtual are best-in-class.
- Team familiarity.
- WebView2's Chromium is a first-class React target.

**Why Vite over Next / CRA:**

- Instant HMR feels great during development.
- Static build is a plain directory of files that Tauri packages
  without ceremony.

## System

| Component     | Version                     | Role                                       |
| ------------- | --------------------------- | ------------------------------------------ |
| **Rust**      | ≥ 1.77.2                    | Compiler toolchain.                        |
| **Node.js**   | ≥ 18                        | Build-time for the frontend bundle.        |
| **npm**       | ≥ 9                         | Package manager.                           |
| **MSVC** Build Tools | 2019 or 2022           | Windows C++ linker for `image`, `sqlite`.  |
| **WebView2 Runtime**  | Evergreen              | Chromium engine hosting the renderer.      |
| **SQLite**    | 3.43+ (bundled via rusqlite)| Local DB with FTS5 `contentless_delete=1`. |

The bundled SQLite is important: FTS5 with `contentless_delete=1` is
a 3.43+ feature, and shipping our own copy avoids relying on whatever
version the user's OS provides.
