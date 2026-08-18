# Interoperability with other tools

PicOrg is designed to be a **good citizen** in a mixed toolchain. It
writes only industry-standard XMP, in two places (sidecar and embedded),
so it plays nice with:

## Windows File Explorer

- **Reads:** PicOrg tags show up in the *Details* tab (right-click a
  photo → Properties → Details), and in the *Tags* column when you
  enable it in a folder view.
- **Writes:** Tags you add through Explorer's Details tab are written
  as `MicrosoftPhoto:LastKeywordXMP` **inside the JPEG**. PicOrg picks
  those up on the next rescan (or when you next open the details
  panel for that photo).

## Windows Photos app

- **Reads:** Titles and ratings written by PicOrg show up.
- **Writes:** Very little; ratings you set in Photos are read by PicOrg
  on next rescan.

## Adobe Lightroom Classic

- **Sidecar path.** Lightroom writes `.xmp` sidecars using exactly the
  same convention PicOrg does (`Photo.CR2` → `Photo.xmp`). Both tools
  read each other's sidecars without a fuss.
- **Standard fields.** `xmp:Rating`, `dc:title`, `dc:description`,
  `dc:subject` are all interoperable.
- **Metadata sync.** If you edit metadata in Lightroom, run
  Metadata → *Save Metadata to File* to force a sidecar update, then
  Rescan in PicOrg. Or the other way around.

## Adobe Bridge

Reads and writes the same XMP that Lightroom does — everything above
applies unchanged.

## digiKam

- **Reads embedded XMP** by default (no sidecar dependency) — PicOrg's
  embedded XMP writes make everything visible immediately.
- Also reads sidecars if configured to. Consistent both ways.

## darktable

Reads `.xmp` sidecars natively (darktable uses sidecars for its own
edit history), so PicOrg tags/ratings/titles show up alongside the
darktable edits. No conflict — the two tools use non-overlapping XMP
namespaces.

## XnView, FastStone, IrfanView, ExifTool

All of these read standard XMP from either the sidecar or the embedded
segment, so PicOrg edits are visible.

## The one field that's slightly non-standard

Windows Explorer historically stores tags in a Microsoft-only field
called `MicrosoftPhoto:LastKeywordXMP` in addition to standard
`dc:subject`. To make sure a tag added in Explorer round-trips
correctly, PicOrg **reads both** and **writes both**. Non-Microsoft
tools happily ignore the extra field.

## The one thing PicOrg doesn't yet do

- **RAW files (CR2, NEF, ARW, DNG, RAF, …):** PicOrg reads their EXIF
  and existing sidecars, but writes only the sidecar — the RAW file
  itself is never modified. This is intentional: modifying RAWs
  in-place is risky, and Lightroom's convention is sidecar-only for
  RAW too.

If you need embedded XMP in a RAW to be visible in a tool that only
reads embedded metadata, use PicOrg's sidecar and then run
`exiftool -tagsfromfile Photo.xmp Photo.CR2` in the same folder.
