# Format sample files

Average-sized synthetic files for every format registered in
[`FormatRegistry`](../../src-tauri/src/core/formats/mod.rs). Used for manual
scan/tag/interop testing without a real photo library.

## Generate

```powershell
py -m pip install pillow reportlab pillow-heif imageio-ffmpeg
py samples/format-samples/generate_all.py
```

Outputs:

- `files/<ext>/sample.<ext>` — primary sample per handler
- `files/<alias>/sample.<alias>` — extension aliases (e.g. `jpeg`, `tif`, `m4v`)
- `manifest.json` — target vs actual size, write support, caveats

Base content: **3000×2000** RGB photo with film grain; **1920×1080 × 6 s**
zooming clips for video (via bundled ffmpeg).

## Caveats

Some containers cannot be encoded natively on all platforms. The generator
falls back to structurally valid placeholders (documented in `manifest.json`):

- **Camera RAW** (`.cr2`, `.nef`, …) — ~22 MB TIFF-shaped blobs, not vendor RAW
- **JPEG 2000 / JPEG XR / PSD / JXL** — may be TIFF/PNG/JPEG placeholders when
  encoders are missing
- **OpenEXR** — TIFF fallback if the `OpenEXR` Python package is not installed

These are sufficient for **library scan** and extension routing tests. They are
not interop references for Lightroom or camera firmware.
