use crate::core::is_processable_by_image_crate;
use crate::error::{AppError, AppResult};
use crate::types::ThumbSize;
use fast_image_resize::{images::Image as FirImage, PixelType, ResizeOptions, Resizer};
use image::{codecs::webp::WebPEncoder, imageops, ImageEncoder, RgbaImage};
use std::fs;
use std::io::BufWriter;
use std::path::{Path, PathBuf};

const THUMB_SIZES: &[ThumbSize] = &[ThumbSize::Small, ThumbSize::Medium];

/// Compute the thumbnail path for a given image id and size.
pub fn thumb_path(cache_dir: &Path, image_id: i64, size: ThumbSize) -> PathBuf {
    let bucket = format!("{:02x}", (image_id as u64) % 256);
    cache_dir
        .join(bucket)
        .join(format!("{}_{}.webp", image_id, size.pixels()))
}

/// Delete cached thumbnails for a given image id. Best-effort; errors are ignored.
pub fn delete_thumbnails(cache_dir: &Path, image_id: i64) {
    for size in [ThumbSize::Small, ThumbSize::Medium, ThumbSize::Large] {
        let p = thumb_path(cache_dir, image_id, size);
        let _ = std::fs::remove_file(&p);
    }
}

/// Generate small + medium thumbnails for the image, if the format is decodable.
/// This is best-effort — failures are logged but not fatal.
pub fn ensure_thumbnails(cache_dir: &Path, src: &Path, image_id: i64) -> AppResult<()> {
    let ext = src
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    if !is_processable_by_image_crate(&ext) {
        // For RAW/HEIC we could later extract embedded previews. Skip for v1.
        return Ok(());
    }

    // Only regenerate if any target is missing.
    if THUMB_SIZES
        .iter()
        .all(|s| thumb_path(cache_dir, image_id, *s).exists())
    {
        return Ok(());
    }

    let img = image::ImageReader::open(src)
        .map_err(|e| AppError::ImageDecode(e.to_string()))?
        .with_guessed_format()
        .map_err(|e| AppError::ImageDecode(e.to_string()))?
        .decode()
        .map_err(|e| AppError::ImageDecode(e.to_string()))?;

    let rgba = img.to_rgba8();

    for &size in THUMB_SIZES {
        let out = thumb_path(cache_dir, image_id, size);
        if out.exists() {
            continue;
        }
        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent)?;
        }
        let target = size.pixels();
        let thumb = resize_rgba(&rgba, target);
        save_webp(&thumb, &out)?;
    }

    Ok(())
}

fn resize_rgba(src: &RgbaImage, target_max: u32) -> RgbaImage {
    let (w, h) = (src.width(), src.height());
    if w <= target_max && h <= target_max {
        return src.clone();
    }
    let (nw, nh) = fit_within(w, h, target_max);
    let src_img = FirImage::from_vec_u8(w, h, src.as_raw().to_vec(), PixelType::U8x4)
        .expect("fir source image build");
    let mut dst_img = FirImage::new(nw, nh, PixelType::U8x4);

    let mut resizer = Resizer::new();
    let _ = resizer.resize(&src_img, &mut dst_img, &ResizeOptions::default());

    RgbaImage::from_vec(nw, nh, dst_img.into_vec()).unwrap_or_else(|| {
        // Fallback via `image` crate if fast_image_resize misbehaves
        imageops::resize(src, nw, nh, imageops::FilterType::Triangle)
    })
}

fn fit_within(w: u32, h: u32, target_max: u32) -> (u32, u32) {
    let scale = (target_max as f64 / w.max(h) as f64).min(1.0);
    let nw = ((w as f64) * scale).round() as u32;
    let nh = ((h as f64) * scale).round() as u32;
    (nw.max(1), nh.max(1))
}

fn save_webp(img: &RgbaImage, path: &Path) -> AppResult<()> {
    let f = fs::File::create(path)?;
    let mut w = BufWriter::new(f);
    let encoder = WebPEncoder::new_lossless(&mut w);
    encoder
        .write_image(img.as_raw(), img.width(), img.height(), image::ExtendedColorType::Rgba8)
        .map_err(|e| AppError::ImageDecode(format!("webp encode: {e}")))?;
    Ok(())
}
