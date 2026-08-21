use crate::core::thumbnail;
use crate::core::AppServices;
use crate::db::queries;
use crate::error::{AppError, AppResult};
use crate::types::ThumbSize;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::State;

/// Returns an absolute file path to a thumbnail; the frontend converts
/// it to an `asset://` URL for display.
#[tauri::command]
pub async fn get_thumb_path(
    services: State<'_, Arc<AppServices>>,
    id: i64,
    size: Option<ThumbSize>,
) -> AppResult<String> {
    let size = size.unwrap_or(ThumbSize::Small);
    let details = queries::get_image_row(&services.db, id)?;
    let source = PathBuf::from(&details.summary.path);

    let path = thumbnail::thumb_path(&services.thumb_cache_dir, id, size);
    if !path.exists() {
        // Generate on demand
        if let Err(e) =
            thumbnail::ensure_thumbnails(&services.thumb_cache_dir, &source, id)
        {
            tracing::debug!(error = %e, id, "thumb gen on demand failed");
        }
    }
    if !path.exists() {
        return Err(AppError::Internal(format!(
            "thumbnail not available for image {}",
            id
        )));
    }
    Ok(path.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn get_image_path(
    services: State<'_, Arc<AppServices>>,
    id: i64,
) -> AppResult<String> {
    let details = queries::get_image_row(&services.db, id)?;
    Ok(details.summary.path)
}
