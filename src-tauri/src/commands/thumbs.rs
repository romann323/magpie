use crate::core::thumbnail;
use crate::core::AppServices;
use crate::db::search;
use crate::error::{AppError, AppResult};
use crate::types::ThumbSize;
use std::sync::Arc;
use tauri::State;

/// Returns an absolute file path to a thumbnail; the frontend converts
/// it to an `asset://` URL for display.
///
/// `id` is a *packed global ID* (see `db::pack_global_id`).
#[tauri::command]
pub async fn get_thumb_path(
    services: State<'_, Arc<AppServices>>,
    id: i64,
    size: Option<ThumbSize>,
) -> AppResult<String> {
    let size = size.unwrap_or(ThumbSize::Small);
    let (_folder_id, _local_id, root, row) = search::get_image_by_gid(&services.pool, id)?
        .ok_or(AppError::ImageNotFound(id))?;
    let source = root.join(&row.rel_path);

    let path = thumbnail::thumb_path(&services.thumb_cache_dir, id, size);
    if !path.exists() {
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
    let (_folder_id, _local_id, root, row) = search::get_image_by_gid(&services.pool, id)?
        .ok_or(AppError::ImageNotFound(id))?;
    Ok(root.join(&row.rel_path).to_string_lossy().to_string())
}
