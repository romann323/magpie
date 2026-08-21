# Out of scope for v1

Explicitly deferred to keep v1 shippable. Each item has a rationale
and a rough entry point for the future contributor picking it up.

## Photo editing

- **No crop, rotate, exposure, or colour correction.** Magpie treats
  pixels as read-only. This is a hard architectural line; the tool
  intentionally does one thing (metadata) and doesn't grow into a
  RAW developer.

## Face and object recognition

- **No auto-tagging based on ML models.** The privacy story is
  simpler when no images ever run through an ML pipeline. Future
  work could offer optional local inference (e.g. running an ONNX
  model against a batch of photos) with a clear opt-in flow.

## Cloud sync

- **No account, no cloud storage integration.** OneDrive and iCloud
  Photos are third-party sync backends the user configures at the OS
  level; Magpie reads whatever the OS has materialised.

## Video

- **Videos aren't listed.** The scanner filters to still-image
  extensions. A future release could add HEVC/MP4 with a separate
  thumbnail pipeline (currently the WebP encoder can't handle video
  frames).

## Sidecar XMP files

- **Magpie does not create `.xmp` sidecar files.** All metadata is
  embedded directly in the source image (JPEG APP1 / PNG iTXt). The
  reader still parses a legacy sidecar authored by an older Magpie
  version or by Lightroom on first scan, but the first successful
  save embeds the merged metadata into the source and deletes the
  sidecar. There is intentionally no "write to sidecar instead"
  fallback and no configuration to enable one.

## Metadata write for RAW / HEIC / TIFF / WebP / GIF / BMP

- The write path currently supports **only JPEG and PNG**. Attempts
  to save metadata on any other format return `AppError::MetadataWrite`
  and are surfaced to the UI as a "this format can't store Magpie
  metadata" toast — Magpie refuses to silently drop the edit or
  fall back to a sidecar.
- Follow-ups (in likely difficulty order): WebP `XMP ` chunk in
  the RIFF container, TIFF tag 700 in the primary IFD, HEIF item
  property, and finally each RAW variant. See
  [`Metadata write path`](../design/metadata-write.md) for where
  new formats plug in — every one of them terminates in
  `atomic_write_bytes` and a `format_supports_embedded_xmp(ext)`
  gate.
- Modifying proprietary RAW containers (`.CR2`, `.CR3`, `.NEF`,
  `.ARW`, `.DNG`, `.RAF`, `.ORF`, `.RW2`, `.SRW`) in place is risky
  and format-specific; a safe implementation would need per-vendor
  handling and is a significant multi-release investment.

## Ratings, colour labels, pick / reject flags

- No UI for star ratings, `xmp:Label` colour labels, or pick /
  reject flags. Fields already exist in the XMP namespace and the
  reader preserves them when foreign tools set them; wiring a UI
  would be a small extension of the metadata patch struct.

## Multi-user / multi-library switching

- v1 assumes one user, one library, per install. Switching libraries
  requires resetting the app data directory.

## In-app full-screen preview

- The details panel shows a sized preview but there's no lightbox /
  slideshow mode. Would be a new React route reading from
  `getImagePath`.

## Undo / redo

- No global undo stack. The safety net is auto-save + Recycle Bin +
  standard XMP interop — the user's edits are never lost, but reverting
  them means editing again.

## Smart collections editor

- The DB schema and Tauri commands for smart collections exist, but
  the UI doesn't expose creation/editing yet.

## Duplicate detection

- Content hashes are computed on scan and stored (`content_hash`
  column), but no duplicate-finder UI ships in v1. A future task
  could add a "Duplicates" filter that groups rows by identical
  hashes.
