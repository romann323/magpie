# Product vision and scope

## Vision

Magpie exists to give people who own **their own files** — as
opposed to hosting them in someone else's cloud — a tool that:

- Reads their files from disk, unchanged, wherever they are.
- Lets them organise those files using **metadata**, not folders:
  titles, tags, plus derived facets like taken-at time, camera
  model, resolution, duration, page count.
- Persists user tags in **standard, interoperable formats**
  (XMP embedded directly inside the source file) whenever the
  container supports it, so nothing is locked into Magpie.
- Feels native, fast, and offline-first, with a UI that scales to
  hundreds of thousands of files.

## Scope for v1

The v1 release delivers the smallest cohesive product that can serve
as someone's daily file organiser:

| Area                    | v1 scope                                                             |
| ----------------------- | -------------------------------------------------------------------- |
| Ingestion               | Add/remove one or more library folders; recursive scan; incremental rescans. |
| Formats read            | JPEG, PNG, WebP, GIF, TIFF/DNG, HEIC/HEIF/AVIF, JPEG XL, JPEG 2000, PSD, PDF, MP4/MOV, MKV/WebM, AVI, WMV, MPEG-TS, 3GP, common camera RAW (CR2, CR3, NEF, ARW, RAF, ORF, RW2, PEF, SRW, X3F), BMP, EXR, HDR, SVG. Legacy XMP sidecars are also read for backward compatibility. |
| Formats written         | JPEG (APP1 XMP), PNG (iTXt XMP), WebP (RIFF `XMP ` chunk), GIF89a (Application Extension XMP). Other formats accept tags into Magpie's library only; the source file is left untouched. |
| Metadata edited         | Title + tags. `xmp:Rating` and `dc:description` written by other tools are preserved but not surfaced. |
| Browsing                | Virtualised grid, sort by taken/added/filename/size.                 |
| Filtering               | Folder, tag, and full-text search (FTS5).                            |
| Batch operations        | Bulk add/remove tags, bulk delete-to-recycle-bin.                    |
| Interoperability        | XMP round-trip with Lightroom, Bridge, digiKam, Windows Explorer for writable formats. |
| Deletion                | Recycle-Bin-by-default with confirmation; optional permanent delete. |
| Storage                 | SQLite index + WebP thumbnail cache under `%APPDATA%`.               |

## Product principles

1. **Never modify pixels.** Metadata edits go into the XMP segment
   or chunk of the file, never into the image data itself.
2. **Never move or rename files.** The user is in charge of their
   folder hierarchy.
3. **No cloud, no telemetry.** Everything runs locally. There are no
   analytics, crash reports, or "phone home" mechanisms.
4. **Fast enough to be daily-drivable.** All hot paths are on native
   Rust; the UI is virtualised; the DB fits in RAM even for
   very large libraries.
5. **Interop over lock-in.** For every format Magpie can write to,
   the user's tags live in the industry-standard XMP form
   Lightroom, Bridge, digiKam, and Windows Explorer understand. For
   the rest, Magpie prints a clear note ("this handler is
   read-only") so the trade-off is never a surprise.
6. **Predictable failure modes.** A partially-broken run is better
   than a silently-lost edit. Every write is atomic, every batch
   op is per-item retryable, and the log records what happened.
