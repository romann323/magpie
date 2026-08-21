use crate::core::metadata::read as meta_read;
use crate::core::metadata::sidecar::sidecar_path_for;
use crate::core::metadata::write as meta_write;
use crate::core::{thumbnail, AppServices};
use crate::db::queries;
use crate::db::queries::ImageDetailsRow;
use crate::error::AppResult;
use crate::types::*;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};

pub const IMAGE_UPDATED_EVENT: &str = "app://image-updated";
pub const IMAGES_DELETED_EVENT: &str = "app://images-deleted";

#[tauri::command]
pub async fn query_images(
    services: State<'_, Arc<AppServices>>,
    filter: Option<ImageFilter>,
    sort: Option<ImageSort>,
    page: Option<Pagination>,
) -> AppResult<Page<ImageSummary>> {
    let filter = filter.unwrap_or_default();
    let sort = sort.unwrap_or_default();
    let page = page.unwrap_or_default();
    queries::query_images(&services.db, &filter, &sort, &page)
}

#[tauri::command]
pub async fn get_image(
    services: State<'_, Arc<AppServices>>,
    id: i64,
) -> AppResult<ImageDetails> {
    let cached = queries::get_image_row(&services.db, id)?;

    // Re-read metadata from disk if the file or its (legacy) sidecar has been
    // modified since we last read metadata.
    let path = PathBuf::from(&cached.summary.path);
    let cached = if refresh_needed_from_fs(&path, &cached) {
        let registry = services.formats.clone();
        let path_bg = path.clone();
        let fresh = tauri::async_runtime::spawn_blocking(move || {
            meta_read::read_all(&registry, &path_bg)
        })
        .await
        .ok()
        .and_then(|r| r.ok());

        if let Some(m) = fresh {
            match queries::resync_user_meta_from_fs(&services.db, id, &m) {
                Ok(()) => {
                    tracing::info!(id, "get_image: resynced user metadata from FS");
                    queries::get_image_row(&services.db, id)?
                }
                Err(e) => {
                    tracing::warn!(id, error = %e, "get_image: FS resync failed");
                    cached
                }
            }
        } else {
            cached
        }
    } else {
        cached
    };

    Ok(enrich_details(services.inner(), cached))
}

/// Wrap a DB row in the full IPC-facing `ImageDetails`, filling in the
/// handler-provided technical metadata and format-info flags. The technical
/// read happens synchronously — files are small headers, EXIF parsing is
/// fast, and the DetailsPanel calls this at most a few times per second.
///
/// `can_write_tags` is `true` if either the native handler embeds tags OR
/// the Windows Shell property system does — the DetailsPanel needs the
/// combined capability so it doesn't grey out fields for RAW/video/PDF
/// files where the write actually will succeed via the Shell fallback.
fn enrich_details(services: &Arc<crate::core::AppServices>, row: ImageDetailsRow) -> ImageDetails {
    let registry = &services.formats;
    let path = std::path::Path::new(&row.summary.path);
    let ext = row.summary.ext.clone();

    let (format_handler, native_can_write, technical) = if let Some(h) = registry.for_ext(&ext) {
        let tech = h.read_technical(path);
        (
            h.name().to_string(),
            h.can_write_tags(),
            tech.as_pairs(),
        )
    } else {
        (String::new(), false, Vec::new())
    };

    // Compute the *effective* write mode. The dispatch in
    // `write_metadata_to_source` mirrors this order exactly — native handler
    // wins over Shell fallback, and both win over library-only.
    let write_mode = if native_can_write {
        crate::types::WriteMode::Native
    } else if services.shell_can_write_tags(path) {
        crate::types::WriteMode::Shell
    } else {
        crate::types::WriteMode::LibraryOnly
    };
    let can_write_tags = !matches!(write_mode, crate::types::WriteMode::LibraryOnly);

    ImageDetails {
        summary: row.summary,
        tags: row.tags,
        meta_written_at: row.meta_written_at,
        meta_read_at: row.meta_read_at,
        technical,
        format_handler,
        can_write_tags,
        write_mode,
    }
}

fn refresh_needed_from_fs(image_path: &Path, cached: &ImageDetailsRow) -> bool {
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
) -> AppResult<ImageDetails> {
    tracing::info!(id, ?patch, "update_image_metadata");
    apply_patch_and_persist(&services, &app_handle, id, &patch).await?;
    let row = queries::get_image_row(&services.db, id)?;
    Ok(enrich_details(services.inner(), row))
}

async fn apply_patch_and_persist(
    services: &Arc<AppServices>,
    app_handle: &AppHandle,
    id: i64,
    patch: &MetadataPatch,
) -> AppResult<()> {
    queries::apply_metadata_patch(&services.db, id, patch)?;

    let row = queries::get_image_row(&services.db, id)?;
    let image_path = PathBuf::from(&row.summary.path);

    let subjects_opt = if patch.tags.is_some()
        || patch.tags_add.is_some()
        || patch.tags_remove.is_some()
    {
        Some(row.tags.clone())
    } else {
        None
    };
    let title_opt = patch.title.clone();

    let registry = services.formats.clone();
    let write_path = image_path.clone();
    let write_res = tauri::async_runtime::spawn_blocking(move || {
        meta_write::write_metadata_to_source(
            &registry,
            &write_path,
            title_opt,
            subjects_opt,
        )
    })
    .await
    .map_err(|e| crate::error::AppError::Internal(format!("metadata write join: {e}")))?;

    match write_res {
        Ok(()) => {
            let now = chrono::Utc::now().timestamp_millis();
            let _ = queries::set_meta_written_at(&services.db, id, now);
            let _ = queries::set_meta_read_at_now(&services.db, id);
            let _ = app_handle.emit(IMAGE_UPDATED_EVENT, id);
            tracing::info!(id, ?image_path, "metadata embedded in source file");
            Ok(())
        }
        Err(e) => {
            tracing::warn!(?image_path, error = %e, "embed write failed");
            Err(e)
        }
    }
}

#[tauri::command]
pub async fn batch_update_metadata(
    services: State<'_, Arc<AppServices>>,
    app_handle: AppHandle,
    ids: Vec<i64>,
    patch: MetadataPatch,
) -> AppResult<Vec<i64>> {
    tracing::info!(count = ids.len(), ?patch, "batch_update_metadata");
    let services = services.inner().clone();
    let mut ok = Vec::with_capacity(ids.len());
    for id in ids {
        match apply_patch_and_persist(&services, &app_handle, id, &patch).await {
            Ok(()) => ok.push(id),
            Err(e) => {
                tracing::warn!(id, error = %e, "batch: apply failed");
            }
        }
    }
    tracing::info!(succeeded = ok.len(), "batch_update_metadata completed");
    Ok(ok)
}

#[tauri::command]
pub async fn delete_images(
    services: State<'_, Arc<AppServices>>,
    app_handle: AppHandle,
    ids: Vec<i64>,
    #[allow(non_snake_case)] permanent: Option<bool>,
) -> AppResult<DeleteResult> {
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
    .map_err(|e| crate::error::AppError::Internal(format!("delete join: {e}")))?;

    if !out.deleted.is_empty() {
        let _ = app_handle.emit(IMAGES_DELETED_EVENT, &out.deleted);
    }

    Ok(out)
}

fn delete_one(path: &Path, permanent: bool) -> Result<(), String> {
    if !path.exists() {
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
