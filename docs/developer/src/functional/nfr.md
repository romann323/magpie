# Non-functional requirements

## Performance

| Metric                                                | Target      | Achieved (release build) |
| ----------------------------------------------------- | ----------- | ------------------------ |
| Cold start to first paint                             | ≤ 1.5 s     | ~0.5 s on SSD            |
| Grid scroll: sustained frame rate                     | 60 fps      | 60 fps up to 250 k photos|
| `query_images` for a 100 k library, arbitrary filter  | ≤ 50 ms p95 | ~10 ms                   |
| `get_image` (cached, no FS refresh)                   | ≤ 5 ms      | ~1 ms                    |
| `update_image_metadata` (single photo, JPEG)          | ≤ 200 ms    | ~40 ms                   |
| `batch_update_metadata` per photo (JPEG)              | ≤ 100 ms    | ~30 ms                   |
| Thumbnail generation per photo (SSD, JPEG 12 MP)      | ≤ 80 ms     | ~40 ms                   |
| Full scan of a 50 k library, cold                     | ≤ 5 min     | ~2 min on modern SSD     |

## Resource limits

- **Memory (idle):** ≤ 250 MB.
- **Memory (10 k photo library, all thumbnails loaded):** ≤ 900 MB.
- **Disk (thumbnail cache):** ≤ 10 KB × #photos (small+medium WebP).
- **Disk (database):** ≤ 1 KB × #photos on average.

## Reliability

- Every metadata mutation is **transactional** at the SQLite level.
  If any part of the transaction fails, none of it lands.
- Every file write (sidecar or embedded XMP) is **atomic** via
  write-to-temp + rename.
- Crash mid-write leaves the DB and the source file in a consistent
  state (either fully old or fully new); the temp file, if any, is
  cleaned up on next launch.
- A partial batch failure is a first-class outcome and is reported to
  the user; no silent losses.

## Compatibility

- **OS:** Windows 10 build 19041+ and Windows 11. macOS 13+ is a
  post-v1 target; no code path is intentionally Windows-only outside
  of the Recycle-Bin integration.
- **Long paths:** All Rust `PathBuf` handling supports `\\?\`-prefixed
  paths natively. Tested end-to-end with 300-character paths.
- **OneDrive:** Files-on-demand (reparse points) are read as their
  underlying content once materialized; PicOrg does not force
  hydration.

## Interoperability

- Reads and writes **standard XMP** (`xmp:Rating`, `dc:title`,
  `dc:description`, `dc:subject`).
- Reads Microsoft-specific tag fields
  (`MicrosoftPhoto:LastKeywordXMP`).
- Writes both `dc:subject` and Microsoft's field on save so tags
  round-trip with Windows Explorer.

## Security

- **No network egress.** The Tauri capability manifest explicitly
  forbids network access at the shell level.
- **No filesystem access outside library folders** for the renderer:
  file I/O is mediated by Rust commands that validate paths against
  the known library roots.
- **CSP** blocks inline scripts and remote origins in the renderer.

## Observability

- File-based rolling log at
  `%APPDATA%\com.picorg.picorg\logs\picorg.log`.
- Structured log fields via `tracing`: `id`, `image_path`,
  `operation`, `duration_ms`.
- Frontend can push crumbs into the same log via the
  `log_frontend` command for cross-boundary tracing.

## Accessibility

- Full keyboard navigation for grid and details panel.
- ARIA labels on every interactive control.
- Focus rings preserved (no `outline: none` shenanigans).
- Contrast ratios ≥ 4.5:1 in both light and dark themes.
