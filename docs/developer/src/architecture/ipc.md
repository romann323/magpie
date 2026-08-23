# IPC boundary

## The contract

The renderer and the Rust core communicate through two mechanisms
provided by Tauri:

1. **Commands** — request/response, from renderer to Rust. Each is a
   Rust `async fn` annotated with `#[tauri::command]` and registered
   in `lib.rs::invoke_handler`.
2. **Events** — one-way, from Rust to renderer, delivered via
   `AppHandle::emit(name, payload)` and listened for via
   `@tauri-apps/api/event::listen`.

Everything is (de)serialised with Serde on the Rust side, JSON on the
wire, and TypeScript types on the frontend side. See
[Tauri command reference](../design/commands.md) for the full list.

## Naming conventions

- **Commands** use `snake_case` (Rust) mapped to a same-name string
  on the frontend: `invoke('update_image_metadata', {…})`.
- **Command arguments** are Rust struct fields in `snake_case`,
  serialised into the JSON payload as-is. The frontend passes them
  as `camelCase` object keys because we set
  `#[serde(rename_all = "camelCase")]` on every argument struct.
- **Events** use a `app://` prefix and a resource-style name:
  `app://image-updated`, `app://images-deleted`,
  `app://scan`.

## Argument struct design

Every command that takes structured data uses a dedicated struct in
`src-tauri/src/types.rs`. Optional fields are `Option<T>`, and any
field where "unset" and "set to null" must be distinguished uses the
custom `double_option` deserializer — an `Option<Option<T>>` where:

| Wire form          | Deserialised as    | Semantics                    |
| ------------------ | ------------------ | ---------------------------- |
| `{ }` (omitted)    | `None`             | Don't touch this field.      |
| `{ "field": null }`| `Some(None)`       | Clear this field explicitly. |
| `{ "field": "x" }` | `Some(Some("x"))`  | Set to `"x"`.                |

This distinction matters for the `MetadataPatch` struct: the
frontend needs to be able to *clear* a title without also unsetting
every other field in the patch.

## Type mirroring

`src/types.ts` mirrors the Rust structs. Both sides intentionally
avoid derived types (`Omit<T, K>`, `Pick<T, K>`) so a schema change
is a single edit in each file.

The frontend types file is a plain declaration file:

```ts
export interface ImageDetails {
  id: number            // packed global ID
  folderId: number
  path: string          // absolute (folder root + rel_path)
  filename: string
  // ...
  tags: string[]
  title: string | null
  importedAt: number    // when the row was first inserted
  technical: Array<[string, string]>
  formatHandler: string
}
```

Every `getImage`, `updateImageMetadata`, etc. wrapper in `ipc.ts`
declares its argument and return types, so a Rust command that
grows a new field lights up a red squiggle in every caller until the
frontend catches up.

## Error handling

- Rust commands return `AppResult<T>` (an alias for
  `Result<T, AppError>`).
- `AppError` is a `thiserror::Error` enum with a `Display` that's
  safe to show to the user (no internal paths in error messages,
  etc.).
- On the wire, errors serialise to a plain string. The frontend
  surfaces them via TanStack Query's `onError` callback and console
  logs.

## Events (Rust → renderer)

| Event                       | Payload                            | Fired when                                     |
| --------------------------- | ---------------------------------- | ---------------------------------------------- |
| `app://scan`             | `ScanProgress { folder_id, done, total, current }` | During a folder scan.  |
| `app://image-updated`    | `i64` (packed global ID)           | After successful metadata patch.               |
| `app://images-deleted`   | `Vec<i64>` (deleted ids)           | After successful delete.                       |

Listeners are attached in `App.tsx` via `useEffect` + `onScanProgress`
etc., and each listener updates the appropriate TanStack Query cache
or triggers a targeted invalidation.

## Capabilities and security

- The renderer has **no direct FS access**. Its `asset:` protocol
  (used to render thumbnails and full images) is scoped to specific
  folders declared in `capabilities/default.json`.
- The `tauri.conf.json` `security.csp` blocks inline scripts,
  remote origins, and `eval`.
- No `dangerouslyUseHttpScheme` or other loosening options are set.
- Commands that operate on paths validate them against the library
  roots before touching the filesystem.
