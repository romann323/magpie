# Preface

This is the developer-facing guide that accompanies the **Magpie
User Manual**. It is organised into three modules:

- **Module 1 — Functional overview.** What Magpie does, for whom, and
  why. Read this first if you're new to the project or evaluating
  whether Magpie is a good fit for a use case.
- **Module 2 — Architecture.** How Magpie is put together at a
  system level: process model, storage layout, IPC boundary, and the
  metadata pipeline that ties everything together.
- **Module 3 — Detailed design.** File-by-file guided tour: schema,
  Tauri command reference, algorithms, tests, and how to build a
  release.

## Conventions used

- Rust file paths are given relative to `src-tauri/`, e.g.
  `src/commands/images.rs`.
- Frontend file paths are relative to the project root, e.g.
  `src/features/DetailsPanel.tsx`.
- Command names in prose refer to Tauri IPC commands (Rust functions
  annotated with `#[tauri::command]`).
- Bold names in schemas refer to primary keys.

## Audience

You are expected to be comfortable with:

- Rust 2021, Tokio async runtime, and Cargo workspaces.
- React 18 with hooks, TypeScript, and TanStack Query.
- SQL (SQLite specifically), including a passing familiarity with FTS5.
- Windows path semantics (`\\?\` prefixes, junctions, OneDrive).

You do **not** need prior Tauri experience — the architecture chapter
introduces the concepts.

## Getting the code and building it

```bash
git clone <repo> magpie
cd magpie
npm install                # frontend deps
cd src-tauri && cargo fetch  # backend deps
cd ..
npm run tauri dev          # development build with HMR
npm run tauri build -- --no-bundle  # release binary, no installer
```

Prerequisites on Windows are Rust ≥ 1.77.2 and the MSVC C++ Build
Tools. See [Build and release](./design/build.md) for the full setup.
