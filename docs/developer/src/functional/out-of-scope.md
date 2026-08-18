# Out of scope for v1

Explicitly deferred to keep v1 shippable. Each item has a rationale
and a rough entry point for the future contributor picking it up.

## Photo editing

- **No crop, rotate, exposure, or colour correction.** PicOrg treats
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
  level; PicOrg reads whatever the OS has materialised.

## Video

- **Videos aren't listed.** The scanner filters to still-image
  extensions. A future release could add HEVC/MP4 with a separate
  thumbnail pipeline (currently the WebP encoder can't handle video
  frames).

## RAW embedded XMP write

- **RAW files get only a sidecar.** Modifying a proprietary RAW
  container in place is risky and format-specific. The safer route
  is to write a sibling `.xmp` that Lightroom, Bridge, and digiKam
  read natively.

## PNG / HEIC / TIFF embedded XMP write

- Only JPEG has an embedded-XMP writer in v1. PNG (iTXt chunk) and
  TIFF (XMP IFD tag) are straightforward extensions; HEIF's chunk
  format is more involved. See
  [`docs/developer/src/design/metadata-write.md`](../design/metadata-write.md)
  for the JPEG implementation and where new formats would plug in.

## Ratings across pick / reject flags

- No support yet for Lightroom-style `xmp:Label` colour labels or
  `pick/reject` flags. Fields exist in the XMP namespace; would be
  a small extension of the metadata patch struct.

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
