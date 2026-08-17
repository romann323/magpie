use std::path::{Path, PathBuf};

/// Compute the standard XMP sidecar path for an image.
/// We follow the Lightroom-compatible convention: `Photo.CR2` → `Photo.xmp`.
/// (Adobe's convention strips the original extension. digiKam uses `Photo.CR2.xmp`.
/// We look for both when reading, but write to the Lightroom-style one by default.)
pub fn sidecar_path_for(image_path: &Path) -> PathBuf {
    image_path.with_extension("xmp")
}

/// Alternative sidecar path (digiKam style, keeping full filename).
#[allow(dead_code)]
pub fn sidecar_path_alt(image_path: &Path) -> PathBuf {
    let mut s = image_path.as_os_str().to_owned();
    s.push(".xmp");
    PathBuf::from(s)
}
