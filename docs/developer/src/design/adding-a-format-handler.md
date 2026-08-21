# Adding a format handler

Adding support for a new file type is a five-step recipe. This page
walks through it using a fictitious "MyFormat" (`.myf`) as an
example.

## 1. Pick the right home

- **Read-only** (Magpie only extracts technical metadata): add a
  new struct to `src-tauri/src/core/formats/stubs.rs` — this file
  already hosts a couple of dozen similar handlers.
- **Read + write** (Magpie can embed tags into the file): create a
  new module `src-tauri/src/core/formats/myformat.rs`.

Both flavours implement the same trait; the only difference is that
a writable handler contains real embedding code in `write_user`,
while a read-only one returns
`common::write_not_supported_error(...)`.

## 2. Implement `FormatHandler`

```rust
use super::common::{self, write_not_supported_error, TechnicalMeta};
use super::{FormatHandler, FormatKind, UserMeta};
use crate::error::AppResult;
use std::path::Path;

pub struct MyFormat;

impl FormatHandler for MyFormat {
    fn name(&self) -> &'static str {
        "MyFormat (read-only)"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["myf"]
    }
    fn kind(&self) -> FormatKind {
        FormatKind::Image
    }
    fn can_write_tags(&self) -> bool {
        false
    }

    fn read_technical(&self, path: &Path) -> TechnicalMeta {
        let mut t = TechnicalMeta::default();
        common::append_file_basics(&mut t, path);
        // ... push more pairs specific to this format ...
        t
    }

    fn read_user(&self, path: &Path) -> AppResult<UserMeta> {
        let _ = path;
        Ok(UserMeta::default())
    }

    fn write_user(&self, path: &Path, _meta: &UserMeta) -> AppResult<()> {
        write_not_supported_error(path, "myf")
    }
}
```

For a **writable** handler, `write_user` typically:

1. Reads the current bytes via `std::fs::read`.
2. Uses `xmp_packet::parse_xmp` to recover any existing XMP.
3. Merges the incoming `UserMeta` into that packet with
   `xmp_packet::merge_user_edits`, preserving foreign fields
   (rating, description, …).
4. Rebuilds the packet with `xmp_packet::build_xmp_packet`.
5. Injects the new packet into the file's format-specific container.
6. Writes the result via `common::atomic_write_bytes` — never
   in-place. `atomic_write_bytes` uses the tmp-file + rename pattern
   so a crash mid-write cannot corrupt the source.

## 3. Register it

Open `src-tauri/src/core/formats/mod.rs` and add the handler to
`FormatRegistry::new`:

```rust
registry.register(Arc::new(myformat::MyFormat));
```

For a read-only stub, add it to `stubs::register` (already called
from `FormatRegistry::new`).

## 4. Write tests

Add a roundtrip test in `src-tauri/tests/metadata_fs.rs`:

```rust
#[test]
fn embed_xmp_roundtrip_myformat() {
    let tmp = tempdir();
    let img = tmp.join("real.myf");
    std::fs::write(&img, tiny_myformat()).unwrap();
    let reg = registry();
    let h = reg.for_ext("myf").unwrap();
    h.write_user(&img, &UserMeta {
        title: Some("Sample".into()),
        tags: vec!["one".into(), "two".into()],
    }).unwrap();
    let after = h.read_user(&img).unwrap();
    assert_eq!(after.title.as_deref(), Some("Sample"));
    assert_eq!(after.tags, vec!["one".to_string(), "two".to_string()]);
}
```

For a read-only handler, add it to
`registry_recognises_every_expected_extension` to prove the
extension is scannable, and test that `write_user` returns an error
that names the format.

## 5. Update the docs

- Add a row to the handler catalogue on
  [File formats](./file-formats.md).
- Add the extension to the user-manual
  [Supported file formats](../../../user-manual/src/file-formats.md)
  page.

## Design invariants

While writing a new handler, keep these invariants in mind:

- **Never create sidecar files.** If you can't embed, return an
  error via `write_not_supported_error`. The frontend gracefully
  handles that and Magpie remembers the tag in its own library
  instead.
- **Never write in-place.** Always go through
  `common::atomic_write_bytes` so the source file can never be left
  half-written.
- **Preserve foreign fields.** When rewriting an XMP packet, use the
  `merge_user_edits` helper — direct `title`/`tags` writes must not
  touch `dc:description`, `xmp:Rating`, GPS, or any other tags Magpie
  doesn't own.
- **Return the extension in error messages.** Callers surface these
  to the UI. Include the offending file extension for a good user
  message (see `write_not_supported_error`).
- **No blocking I/O on hot paths.** Handlers may be invoked from the
  scanner (parallel) and the metadata write path (foreground);
  restrict work in `read_technical` to what's cheap enough to run
  during a scan.
