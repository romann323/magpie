# Adding a format handler

Format handlers are the plug-in point for supporting a new file
type. **All handlers are read-only** after the DB redesign — Magpie
never writes back into source files. See
[Database redesign](./db-redesign.md).

This page walks through the four-step recipe using a fictitious
"MyFormat" (`.myf`) as an example.

## 1. Pick the right home

- Add a new struct to `src-tauri/src/core/formats/stubs.rs` if the
  handler only pulls technical metadata via `imagesize` / magic
  bytes, or
- Create a dedicated module `src-tauri/src/core/formats/myformat.rs`
  if you need a real parser (like `jpeg.rs`, `png.rs`, `tiff.rs`).

## 2. Implement `FormatHandler`

```rust
use super::common::{self, TechnicalMeta};
use super::{FormatHandler, FormatKind, UserMeta};
use crate::error::AppResult;
use std::path::Path;

pub struct MyFormat;

impl FormatHandler for MyFormat {
    fn name(&self) -> &'static str {
        "MyFormat"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["myf"]
    }
    fn kind(&self) -> FormatKind {
        FormatKind::Image
    }

    fn read_technical(&self, path: &Path) -> TechnicalMeta {
        let mut t = TechnicalMeta::default();
        common::append_file_basics(&mut t, path);
        // ... push more pairs specific to this format ...
        t
    }

    fn read_user(&self, path: &Path) -> AppResult<UserMeta> {
        // Native handlers can parse an embedded XMP packet here.
        // Stubs that rely on the Windows Shell fallback just
        // return an empty result and let read_all pick up the
        // slack.
        let _ = path;
        Ok(UserMeta::default())
    }
}
```

That's the whole trait. There's no `write_user`, no
`can_write_tags`. If a file's format ever needs bespoke read-only
handling of embedded tags (say, a proprietary MP4 atom), do it
inside `read_user` — that's the only user-metadata seam we still
expose.

## 3. Register it

Open `src-tauri/src/core/formats/mod.rs` and add the handler to
`FormatRegistry::new`:

```rust
registry.register(Arc::new(myformat::MyFormat));
```

For a stub, add it to `stubs::register` (already called from
`FormatRegistry::new`).

## 4. Write tests

Add a scanner-facing test that confirms:

- Files with the new extension are picked up by the walk.
- `handler.read_technical(path)` returns sensible values for a
  minimal fixture.
- `handler.read_user(path)` returns the tags Magpie should import
  on first sight (if the format supports embedded metadata).

The `registry_recognises_every_expected_extension` test in
`src-tauri/tests/format_registry.rs` is the right place to prove
the extension is known.

## 5. Update the docs

- Add a row to the handler catalogue on
  [File formats](./file-formats.md).
- Add the extension to the user-manual
  [Supported file formats](../../../user-manual/src/file-formats.md)
  page.

## Design invariants

While writing a new handler, keep these invariants in mind:

- **Never write to the source file.** The DB is the source of
  truth for tags and titles. Any hypothetical need to write back
  should be discussed as an architectural change, not sneaked into
  a handler.
- **No blocking I/O on hot paths.** Handlers are invoked from the
  scanner (parallel) and the `get_image` command (foreground);
  restrict work in `read_technical` to what's cheap enough to run
  during a scan.
- **Failures are non-fatal.** If reading a specific field fails,
  return an empty `UserMeta` / partial `TechnicalMeta` — the caller
  will still insert the row with whatever succeeded.
