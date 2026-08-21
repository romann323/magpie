# Testing strategy

## Layers

```
   ┌────────────────────────────────────────────────────────┐
   │  L4: Manual smoke — screenshot scripts + real UI clicks│  ←  rare
   ├────────────────────────────────────────────────────────┤
   │  L3: Integration tests (Rust)                          │  ←  ~5s per run
   │      src-tauri/tests/metadata_fs.rs                    │
   ├────────────────────────────────────────────────────────┤
   │  L2: Unit tests (Rust, inline #[cfg(test)])            │  ←  <1s
   │      xmp.rs (parse/build/CRC), migrations.rs           │
   ├────────────────────────────────────────────────────────┤
   │  L1: Type checks (TypeScript strict + cargo check)     │  ←  seconds
   └────────────────────────────────────────────────────────┘
```

## L1 — type checks

- `cargo check --workspace` for the Rust side (no unused warnings
  allowed in CI).
- `tsc -b --pretty false` for the frontend; strict mode enabled in
  `tsconfig.json` (`noUncheckedIndexedAccess`, `noImplicitAny`,
  `strictNullChecks`).

Both are cheap and run before every commit.

## L2 — Rust unit tests

Colocated with modules in `#[cfg(test)] mod tests { … }`. Examples:

- `core/metadata/xmp.rs` — parser tests for Windows Explorer tags,
  Microsoft-only keyword lists, case variants, attribute-form
  values. Also covers the JPEG APP1 walker and (in integration
  tests) the PNG iTXt CRC.
- `db/queries.rs` — smoke tests for FTS row rebuild.

Run with:

```powershell
cd src-tauri
cargo test --lib
```

## L3 — Rust integration tests

`src-tauri/tests/metadata_fs.rs` exercises the full pipeline on
temporary directories:

| Test                                              | Verifies                                            |
| ------------------------------------------------- | --------------------------------------------------- |
| `read_sidecar_end_to_end`                         | Legacy sidecar read: title + tags.                  |
| `read_sidecar_case_variants`                      | XMP parser is namespace/case insensitive.           |
| `fts_delete_after_tag_update_works`               | The FTS5 `contentless_delete` regression is fixed.  |
| `batch_tag_add_persists_for_every_image`          | Bulk `tags_add` / `tags_remove` semantics.          |
| `embed_xmp_roundtrip_jpeg`                        | JPEG embed writes, re-reads, replaces on second write. |
| `embed_xmp_roundtrip_png`                         | PNG iTXt embed, no chunk stacking on rewrite.       |
| `embed_xmp_roundtrip_webp`                        | WebP RIFF `XMP ` chunk roundtrip, no stacking.      |
| `embed_xmp_roundtrip_gif`                         | GIF89a Application Extension XMP roundtrip.         |
| `write_never_creates_sidecar_for_jpeg`            | Saving on a JPEG creates zero `.xmp` files.         |
| `write_removes_legacy_sidecar_after_embed`        | A pre-existing `.xmp` is removed on successful embed. |
| `write_preserves_foreign_rating_and_description`  | Foreign `xmp:Rating` and `dc:description` survive a tag edit. |
| `write_errors_on_unsupported_format`              | Saving on RAW returns `Err`, no sidecar fallback.   |
| `registry_recognises_every_expected_extension`    | Every advertised handler is registered.             |

Run:

```powershell
cd src-tauri
cargo test --test metadata_fs
```

## L4 — screenshot smoke tests

Under `scripts/`:

- `screenshot-app.ps1` — launches Magpie, finds its window with
  `EnumWindows`, screenshots via `PrintWindow`
  (`PW_RENDERFULLCONTENT`) so the shot works even if another window
  is in front.
- `screenshot-scrolled.ps1` — clicks an image, scrolls the details
  panel, screenshots.
- `test-multiselect-tag.ps1` — end-to-end skeleton for Ctrl+click
  → type tag → click Apply. UI automation is inherently flaky on
  Windows; use these mostly to verify a build launches and paints,
  not for asserting behaviour.

These are not part of automated CI. They're one-off tools when
investigating a UI-only bug.

## Frontend tests

Currently minimal:

- Type safety is the main safety net.
- Component tests would use Vitest + React Testing Library; not
  set up in v1.

Because the UI is a thin translation over Rust commands, the
interesting behaviour is on the Rust side and covered by L2/L3.

## Format sample files

`samples/format-samples/` holds one average-sized synthetic file per
handler registered in `FormatRegistry` (35 handlers, plus extension
aliases such as `.jpeg`, `.tif`, `.m4v`).

Generate or refresh:

```powershell
py -m pip install pillow reportlab pillow-heif imageio-ffmpeg
py samples/format-samples/generate_all.py
```

Output lands in `samples/format-samples/files/<ext>/sample.<ext>` with
sizes and placeholder notes recorded in `manifest.json`. See
`samples/format-samples/README.md` for caveats (camera RAW and some
exotic codecs are scan-only placeholders, not vendor-native files).

Use these when manually exercising the scanner, thumbnail pipeline, or
details panel against a broad extension set without copying a real
library onto the machine.

## Diagnostic tools (not tests, but adjacent)

- `src-tauri/examples/dump_meta.rs` — prints filesystem state,
  embedded XMP, any legacy sidecar contents, parsed metadata, and
  DB state for one photo or the first N rows in the DB. Great for
  reproducing bugs.
- `src-tauri/examples/dump_tag_usage.rs` — given a tag name, list
  every photo carrying it plus what the on-disk embedded XMP shows.
  Used in the "did the batch save actually work?" investigation.

Run:

```powershell
cd src-tauri
cargo run --example dump_meta -- "C:\Photos\IMG_1234.jpg"
```

## CI (recommended)

Not shipped in the repo yet, but the natural setup is:

- Windows runner (matrix: `windows-2022`).
- Steps: `cargo fmt --check`, `cargo clippy --all-targets`,
  `cargo test --workspace`, `npm ci`, `tsc -b`, `npm run tauri build
  -- --no-bundle`.
- Artifacts: `desktop.exe` for smoke inspection.
