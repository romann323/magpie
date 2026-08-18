# Process and threading model

## One process, two runtimes

Tauri gives us **one OS process** with:

- **The main thread** — owns the event loop and the WebView2 host.
- **A Tokio runtime** (`tauri::async_runtime`) driving all async
  Tauri commands.
- **Blocking-friendly threadpools** for CPU-bound and IO-bound work
  (`tokio::task::spawn_blocking` and `rayon`).
- **The WebView2 renderer** on its own thread, hosting the React
  bundle.

No separate Node process, no plugin sidecar, no IPC over sockets.
Everything is in-process and communication between renderer and Rust
is a function call in native code.

## Threads and pools

| Where                        | Rust ownership                          | Typical work                                  |
| ---------------------------- | --------------------------------------- | --------------------------------------------- |
| Main thread                  | Tauri app loop                          | Window events, menu, plugin dispatch          |
| Tokio worker threads         | `tauri::async_runtime` (multi-threaded) | All `#[tauri::command] async fn` bodies       |
| `spawn_blocking` pool        | Tokio's built-in blocking pool          | Sidecar/embed writes, EXIF read, thumbnails   |
| `rayon` pool                 | Global rayon pool                       | Parallel directory walk, batched hashing      |
| Renderer thread              | WebView2                                | React reconciler, layout, paint               |

Every Tauri command runs on a Tokio worker. A command that does
CPU-bound work (encoding a thumbnail, injecting XMP into a 20 MB
JPEG) wraps that in `tauri::async_runtime::spawn_blocking(move || …)`
so the worker isn't blocked.

## SQLite concurrency

The DB is a single `Mutex<Connection>`. Reads and writes serialise
through the lock. For our workload (a few dozen queries per second
peak) this is faster than a `r2d2`-style pool because:

- SQLite is single-writer anyway (writes queue at the file level).
- Our reads are short (`< 5 ms`); lock contention is negligible.
- No connection setup / teardown per query.

Long-running queries that would otherwise hold the lock (like a full
tag-count refresh across 250 k photos) are structured as one
transaction that reads and returns — no cursor kept open — so the
lock is released quickly.

## Async model

- `async fn` at the Tauri command boundary is idiomatic. It lets us
  `await` a background thumbnail generation or a sidecar write
  without blocking a Tokio worker.
- Inside a command, DB operations are synchronous (they take a
  `MutexGuard`). This is deliberate: SQLite calls are fast enough to
  keep on the worker thread, and it removes a whole class of
  cancellation bugs.
- **Rule of thumb:** if a call could block > 5 ms
  (`std::fs::File::read_to_end` of a JPEG, `spawn_blocking` it.
  Otherwise, run it inline.

## Cancellation and shutdown

- Tauri commands don't get an explicit cancellation token; they run
  to completion. Long batch operations are structured as a loop over
  IDs so partial progress is durable even if the user closes the
  window.
- On close, Tauri drops the main window; pending `spawn_blocking`
  tasks are given a grace period to finish. Any in-flight sidecar
  write is atomic (write-to-temp + rename), so worst case a temp
  file is left behind and cleaned up on the next successful write.

## Frontend concurrency

- React Query owns all in-flight IPC promises. It handles
  deduplication (two components asking for `['image', 5]` share the
  underlying `invoke` call) and stale-while-revalidate refetches on
  focus.
- Zustand is synchronous. No middleware, no effects — pure UI state.
- No web workers in v1. The renderer stays responsive because it
  offloads every non-trivial computation to Rust.
