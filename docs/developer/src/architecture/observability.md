# Observability

## File-based logging

Every run of Magpie produces a rolling log file at:

```
%APPDATA%\com.magpie.app\logs\app.log
```

The logger is initialised in `src-tauri/src/lib.rs::init_logging`,
using `tracing-subscriber` with a plain-text formatter and a
`RUST_LOG`-compatible env filter (default: `info,desktop_lib=info`).

Format of a typical line:

```
2026-08-17T18:53:12.331332Z  INFO desktop_lib: Magpie started app_data_dir="…"
2026-08-17T19:22:14.029142Z  INFO desktop_lib::commands::images: get_image: resynced user metadata from FS id=5
2026-08-18T08:41:03.912483Z  INFO desktop_lib::commands::images: batch_update_metadata count=3 patch=…
```

- ISO-8601 timestamp with microsecond precision.
- Log level: `TRACE / DEBUG / INFO / WARN / ERROR`.
- Target: usually the module path (`desktop_lib::commands::images`).
- Structured fields on the tail: `id=5`, `count=3`, `?patch`.

## Structured fields

We use `tracing`'s field syntax pervasively:

```rust
tracing::info!(
    id,
    ?patch,                       // Debug-formatted
    op = "update_image_metadata", // static label
    "metadata embedded in source file"
);
```

The `?` prefix invokes `Debug`. Named fields keep the log
grep-friendly: to find every batch save, `rg 'batch_update_metadata'`
returns just the events, and the id/count/count-succeeded triplet
is on the same line.

## Frontend crumbs into the same log

The renderer can push log lines into the Rust log via the
`log_frontend` command (`src-tauri/src/commands/diag.rs`). The
frontend helper `logFrontend(level, msg)` in `src/ipc.ts` is a
best-effort fire-and-forget:

```ts
logFrontend('info', `applyTags dispatch: ids=${ids.length} add=[…]`)
```

Renderer-side events land in the log with a `target="frontend"`
marker, which makes them easy to filter:

```
2026-08-18T08:41:03.900001Z  INFO frontend: applyTags dispatch: ids=3 add=[vacation] remove=[]
```

This lets us reason about the full IPC round-trip from a single
tail of `app.log`.

## What's logged where

| Signal                                         | Level  | Emitted by                                                       |
| ---------------------------------------------- | ------ | ---------------------------------------------------------------- |
| App started / logging initialised              | INFO   | `lib.rs::init_logging`, `lib.rs` main run                        |
| Legacy layout migrated into magpie.db          | INFO   | `db/migrate.rs::open_or_migrate`                                 |
| Folder registered                              | INFO   | `commands/library.rs::add_library_folder`                        |
| Folder root unreachable (drive unplugged)      | WARN   | `commands/library.rs::list_library_folders`                      |
| Command entry (`update_image_metadata`, …)     | INFO   | Each Tauri command that mutates state                            |
| `get_image` FS-refresh triggered               | INFO   | `commands/images.rs::get_image`                                  |
| Scan errors (unreadable file, permission)      | WARN   | `core/scanner.rs`                                                |
| DB errors (mutex poisoned, statement failed)   | ERROR  | `db/mod.rs`, `db/queries.rs`, `lib.rs::run`                      |
| Frontend applyTags dispatch                    | INFO   | `features/DetailsPanel.tsx::MultiDetails.applyTags`              |

## Log rotation

The current implementation is a plain append-only file — no rotation.
For a v1 release this is fine; the log grows a few kilobytes per
session at INFO. A future PR should add `tracing-appender::rolling`
with daily rotation or size-based (e.g. 10 MB × 5).

## Turning up the volume

Set the `RUST_LOG` environment variable before launching:

```powershell
$env:RUST_LOG = "debug,desktop_lib=trace"
& "$env:LOCALAPPDATA\Programs\Magpie\desktop.exe"
```

This will produce a torrent of scanner-level detail, useful when
debugging a bad scan on a specific folder.

## No telemetry

There is no automatic upload of the log, no crash reporter, no
analytics. `app.log` is on your disk and stays there unless you
explicitly share it (e.g. attach it to a bug report).
