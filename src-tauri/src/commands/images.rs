use crate::core::metadata::read as meta_read;
use crate::core::metadata::sidecar::sidecar_path_for;
use crate::core::metadata::write as meta_write;
use crate::core::{thumbnail, AppServices};
use crate::db::queries;
use crate::error::PicOrgResult;
use crate::types::*;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};

pub const IMAGE_UPDATED_EVENT: &str = "picorg://image-updated";
pub const IMAGES_DELETED_EVENT: &str = "picorg://images-deleted";

#[tauri::command]
pub async fn query_images(
    services: State<'_, Arc<AppServices>>,
    filter: Option<ImageFilter>,
    sort: Option<ImageSort>,
    page: Option<Pagination>,
) -> PicOrgResult<Page<ImageSummary>> {
    let filter = filter.unwrap_or_default();
    let sort = sort.unwrap_or_default();
    let page = page.unwrap_or_default();
    queries::query_images(&services.db, &filter, &sort, &page)
}

#[tauri::command]
pub async fn get_image(
    services: State<'_, Arc<AppServices>>,
    id: i64,
) -> PicOrgResult<ImageDetails> {
    // Pull the DB row first so we know the file path and last-read timestamp.
    let cached = queries::get_image(&services.db, id)?;

    // Re-read metadata from disk if the image file or its sidecar has been
    // modified since we last read metadata. This picks up tags/ratings set
    // externally (Windows Explorer, digiKam, Lightroom, manually edited .xmp).
    let path = PathBuf::from(&cached.summary.path);
    if refresh_needed_from_fs(&path, &cached) {
        let path_bg = path.clone();
        let fresh = tauri::async_runtime::spawn_blocking(move || {
            meta_read::read_all(&path_bg)
        })
        .await
        .ok()
        .and_then(|r| r.ok());

        if let Some(m) = fresh {
            match queries::resync_user_meta_from_fs(&services.db, id, &m) {
                Ok(()) => {
                    tracing::info!(id, "get_image: resynced user metadata from FS");
                    return queries::get_image(&services.db, id);
                }
                Err(e) => {
                    tracing::warn!(id, error = %e, "get_image: FS resync failed");
                }
            }
        }
    }

    Ok(cached)
}

fn refresh_needed_from_fs(image_path: &Path, cached: &ImageDetails) -> bool {
    let last_read = cached.meta_read_at.unwrap_or(0);
    let img_mtime_ms = std::fs::metadata(image_path)
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    if img_mtime_ms > last_read {
        return true;
    }

    let sidecar = sidecar_path_for(image_path);
    let sidecar_mtime_ms = std::fs::metadata(&sidecar)
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    sidecar_mtime_ms > last_read
}

#[tauri::command]
pub async fn update_image_metadata(
    services: State<'_, Arc<AppServices>>,
    app_handle: AppHandle,
    id: i64,
    patch: MetadataPatch,
) -> PicOrgResult<ImageDetails> {
    tracing::info!(id, ?patch, "update_image_metadata");
    apply_patch_and_write_sidecar(&services, &app_handle, id, &patch).await?;
    // Refetch to include the updated meta_written_at / meta_read_at.
    let details = queries::get_image(&services.db, id)?;
    Ok(details)
}

/// Applies a metadata patch to a single image: updates the DB, then writes
/// the sidecar file synchronously. Emits `picorg://image-updated` on success.
///
/// Returns Err only if the DB update or the spawn_blocking join fails; a
/// sidecar-write failure is logged and does NOT roll back the DB (the intent
/// is that the user's edit is captured somewhere even if the source folder is
/// read-only).
async fn apply_patch_and_write_sidecar(
    services: &Arc<AppServices>,
    app_handle: &AppHandle,
    id: i64,
    patch: &MetadataPatch,
) -> PicOrgResult<()> {
    queries::apply_metadata_patch(&services.db, id, patch)?;

    // Refetch to figure out the *final* metadata state and write it to disk.
    let details = queries::get_image(&services.db, id)?;

    let image_path = PathBuf::from(&details.summary.path);
    let subjects_opt = if patch.tags.is_some()
        || patch.tags_add.is_some()
        || patch.tags_remove.is_some()
    {
        Some(details.tags.clone())
    } else {
        None
    };
    let title_opt = patch.title.clone();
    let comment_opt = patch.comment.clone();
    let rating_opt = patch.rating;

    let write_path = image_path.clone();
    let write_res = tauri::async_runtime::spawn_blocking(move || {
        meta_write::merge_and_write_sidecar(
            &write_path,
            title_opt,
            comment_opt,
            rating_opt,
            subjects_opt,
        )
    })
    .await
    .map_err(|e| crate::error::PicOrgError::Internal(format!("sidecar join: {e}")))?;

    match write_res {
        Ok(()) => {
            let now = chrono::Utc::now().timestamp_millis();
            let _ = queries::set_meta_written_at(&services.db, id, now);
            // Refresh `meta_read_at` too so the FS-refresh check in get_image
            // won't try to re-read a file we just wrote.
            let _ = queries::set_meta_read_at_now(&services.db, id);
            let _ = app_handle.emit(IMAGE_UPDATED_EVENT, id);
            tracing::info!(id, ?image_path, "metadata + sidecar saved");
        }
        Err(e) => {
            tracing::warn!(?image_path, error = %e, "sidecar write failed");
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn batch_update_metadata(
    services: State<'_, Arc<AppServices>>,
    app_handle: AppHandle,
    ids: Vec<i64>,
    patch: MetadataPatch,
) -> PicOrgResult<Vec<i64>> {
    tracing::info!(count = ids.len(), ?patch, "batch_update_metadata");
    let services = services.inner().clone();
    let mut ok = Vec::with_capacity(ids.len());
    for id in ids {
        match apply_patch_and_write_sidecar(&services, &app_handle, id, &patch).await {
            Ok(()) => ok.push(id),
            Err(e) => {
                tracing::warn!(id, error = %e, "batch: apply failed");
            }
        }
    }
    tracing::info!(
        succeeded = ok.len(),
        "batch_update_metadata completed"
    );
    Ok(ok)
}

/// Delete image files from disk (Recycle Bin by default) and remove them
/// from the library index. Best-effort: partial failure returns per-file
/// error details.
#[tauri::command]
pub async fn delete_images(
    services: State<'_, Arc<AppServices>>,
    app_handle: AppHandle,
    ids: Vec<i64>,
    #[allow(non_snake_case)] permanent: Option<bool>,
) -> PicOrgResult<DeleteResult> {
    let permanent = permanent.unwrap_or(false);
    tracing::info!(count = ids.len(), permanent, "delete_images");

    let entries = queries::get_image_paths(&services.db, &ids)?;
    let services_clone = services.inner().clone();

    let out = tauri::async_runtime::spawn_blocking(move || {
        let mut deleted: Vec<i64> = Vec::new();
        let mut failed: Vec<DeleteFailure> = Vec::new();

        for (id, path_str) in entries {
            let path = PathBuf::from(&path_str);
            match delete_one(&path, permanent) {
                Ok(()) => {
                    thumbnail::delete_thumbnails(&services_clone.thumb_cache_dir, id);
                    deleted.push(id);
                }
                Err(e) => {
                    failed.push(DeleteFailure {
                        id,
                        path: path_str,
                        error: e,
                    });
                }
            }
        }

        if !deleted.is_empty() {
            if let Err(e) = queries::delete_image_rows(&services_clone.db, &deleted) {
                tracing::warn!(error = %e, "failed to remove deleted rows from DB");
            }
        }

        DeleteResult { deleted, failed }
    })
    .await
    .map_err(|e| crate::error::PicOrgError::Internal(format!("delete join: {e}")))?;

    if !out.deleted.is_empty() {
        let _ = app_handle.emit(IMAGES_DELETED_EVENT, &out.deleted);
    }

    Ok(out)
}

fn delete_one(path: &Path, permanent: bool) -> Result<(), String> {
    if !path.exists() {
        // File already gone - still delete the sidecar and DB row.
        try_delete_sidecar(path, permanent);
        return Ok(());
    }
    if permanent {
        std::fs::remove_file(path).map_err(|e| e.to_string())?;
    } else {
        trash::delete(path).map_err(|e| e.to_string())?;
    }
    try_delete_sidecar(path, permanent);
    Ok(())
}

fn try_delete_sidecar(image_path: &Path, permanent: bool) {
    let sidecar = sidecar_path_for(image_path);
    if sidecar.exists() {
        let r = if permanent {
            std::fs::remove_file(&sidecar).map_err(|e| e.to_string())
        } else {
            trash::delete(&sidecar).map_err(|e| e.to_string())
        };
        if let Err(e) = r {
            tracing::warn!(?sidecar, error = %e, "sidecar delete failed");
        }
    }
}
