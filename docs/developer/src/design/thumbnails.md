# Thumbnail pipeline

## Sizes

Two cached sizes per image, both stored as WebP quality 80:

| Enum name | Long-edge px | Used by                                    |
| --------- | ------------ | ------------------------------------------ |
| `Small`   | ~256         | `ImageGrid` tiles (main use case).         |
| `Medium`  | ~512         | (Reserved for a future zoom-in mode.)      |
| `Large`   | ~1024        | (Currently only generated on request.)     |

Aspect ratio is preserved — the short edge is proportional. A 6000×4000
source becomes a 256×170 small and 512×341 medium.

## Location

Files live in `%APPDATA%\com.magpie.app\thumbs\`:

```
<id>-small.webp
<id>-medium.webp
```

Using the image id (rather than a hash of the source path) keeps the
cache invariant across file renames: as long as Magpie still knows
about the image, the thumbnail is reusable.

## Generation

```rust
pub fn ensure_thumbnails(cache_dir: &Path, src: &Path, image_id: i64) -> Result<()> {
    let src_mtime = fs::metadata(src)?.modified()?;
    for size in &[ThumbSize::Small, ThumbSize::Medium] {
        let out = thumb_path(cache_dir, image_id, *size);
        if is_fresh(&out, src_mtime) { continue; }
        let img = image::open(src)?;
        let target = pick_target_size(&img, *size);
        let thumb = resize_rgba(&img.to_rgba8(), target);
        save_webp(&thumb, &out)?;
    }
    Ok(())
}
```

Details:

- `image::open` handles JPEG, PNG, GIF, TIFF, WebP, HEIC (with the
  `image` crate's HEIC feature enabled). For unsupported RAW
  formats, `open` returns an error and the thumbnail is skipped
  (grid renders a placeholder).
- `resize_rgba` uses `fast_image_resize::Resizer` with the
  `Lanczos3` filter. On x86_64 this dispatches to SSE4/AVX2 SIMD
  automatically.
- `save_webp` writes to `<out>.tmp` then renames — atomic, so a
  crash mid-encode never leaves a corrupt WebP.

## Freshness

A thumbnail is considered fresh iff its file mtime is ≥ the source
file's mtime. Editing metadata does not change the source mtime
enough to invalidate a thumbnail (unless Magpie's embed-XMP write
touched the JPEG — in which case the thumbnail regenerates on next
scan, which is correct because the JPEG has actually changed).

## Deletion

`delete_thumbnails(cache_dir, image_id)` is called from
`delete_images`. It removes every size:

```rust
for size in [Small, Medium, Large] {
    let _ = fs::remove_file(thumb_path(cache_dir, image_id, size));
}
```

Errors are ignored — a missing thumb is the desired state.

## Cache sizing

The `Small` WebP averages 4–8 KB per photo; `Medium` averages
16–32 KB. Total cache size on a 50 000-photo library is roughly
1–2 GB.

There's no automatic eviction in v1. Users who want to trim the
cache can delete `%APPDATA%\com.magpie.app\thumbs\` — Magpie
regenerates on demand.

## Async model

`ensure_thumbnails` is CPU-bound (decode + resize + encode). It's
called from Tauri commands via `tauri::async_runtime::spawn_blocking`
so it doesn't stall a Tokio worker. The scanner enqueues thumb
tasks on the rayon pool to overlap I/O with compute.
