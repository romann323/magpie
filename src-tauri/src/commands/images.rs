use crate::core::metadata::read as meta_read;
use crate::core::{thumbnail, AppServices};
use crate::db;
use crate::db::library::{self, MetadataPatch as LibMetadataPatch};
use crate::db::search;
use crate::error::{AppError, AppResult};
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
    search::query_images(&services.pool, &filter, &sort, &page)
}

#[tauri::command]
pub async fn get_image(
    services: State<'_, Arc<AppServices>>,
    id: i64,
) -> AppResult<ImageDetails> {
    // Locate the row.
    let (folder_id, local_id, root, mut row) = search::get_image_by_gid(&services.pool, id)?
        .ok_or(AppError::ImageNotFound(id))?;

    // Re-read metadata from disk if the file has changed since last import.
    // (After the redesign we don't track meta_read_at anymore because the DB
    // is the source of truth. We compare mtime to what's cached.)
    let abs = root.join(&row.rel_path);
    if let Ok(fs_meta) = std::fs::metadata(&abs) {
        if let Ok(m) = fs_meta.modified() {
            if let Ok(d) = m.duration_since(std::time::UNIX_EPOCH) {
                let disk_mtime = d.as_millis() as i64;
                if disk_mtime > row.mtime_ms {
                    let registry = services.formats.clone();
                    let abs_bg = abs.clone();
                    let fresh = tauri::async_runtime::spawn_blocking(move || {
                        meta_read::read_all(&registry, &abs_bg)
                    })
                    .await
                    .ok()
                    .and_then(|r| r.ok());
                    if let Some(fresh) = fresh {
                        let lib = services.pool.library(folder_id)?;
                        let mut conn = lib.lock()?;
                        library::set_image_meta(&mut conn, local_id, &fresh)?;
                        drop(conn);
                        // Reload the row so downstream sees the fresh values.
                        let lib = services.pool.library(folder_id)?;
                        let conn = lib.lock()?;
                        if let Some(refreshed) = library::get_image_row(&conn, local_id)? {
                            row = refreshed;
                        }
                    }
                }
            }
        }
    }

    Ok(enrich_details(services.inner(), folder_id, root, row))
}

fn enrich_details(
    services: &Arc<AppServices>,
    folder_id: i64,
    root: PathBuf,
    row: library::ImageRow,
) -> ImageDetails {
    let abs = root.join(&row.rel_path);
    let ext = row.ext.clone();
    let path = abs.as_path();

    let (format_handler, technical) = if let Some(h) = services.formats.for_ext(&ext) {
        let tech = h.read_technical(path);
        (h.name().to_string(), tech.as_pairs())
    } else {
        (String::new(), Vec::new())
    };

    ImageDetails {
        summary: ImageSummary {
            id: db::pack_global_id(folder_id, row.local_id),
            folder_id,
            path: abs.to_string_lossy().to_string(),
            filename: row.filename,
            ext,
            width: row.width,
            height: row.height,
            size_bytes: row.size_bytes,
            mtime_ms: row.mtime_ms,
            taken_at: row.taken_at,
            title: row.title,
            content_hash: row.content_hash,
        },
        tags: row.tags,
        technical,
        format_handler,
        imported_at: row.imported_at,
    }
}

#[tauri::command]
pub async fn update_image_metadata(
    services: State<'_, Arc<AppServices>>,
    app_handle: AppHandle,
    id: i64,
    patch: MetadataPatch,
) -> AppResult<ImageDetails> {
    tracing::info!(id, ?patch, "update_image_metadata");
    let lib_patch = LibMetadataPatch {
        title: patch.title.clone(),
        tags: patch.tags.clone(),
        tags_add: patch.tags_add.clone(),
        tags_remove: patch.tags_remove.clone(),
    };
    search::apply_metadata_patch_by_gid(&services.pool, id, &lib_patch)?;
    let _ = app_handle.emit(IMAGE_UPDATED_EVENT, id);
    let (folder_id, _local_id, root, row) = search::get_image_by_gid(&services.pool, id)?
        .ok_or(AppError::ImageNotFound(id))?;
    Ok(enrich_details(services.inner(), folder_id, root, row))
}

#[tauri::command]
pub async fn batch_update_metadata(
    services: State<'_, Arc<AppServices>>,
    app_handle: AppHandle,
    ids: Vec<i64>,
    patch: MetadataPatch,
) -> AppResult<Vec<i64>> {
    tracing::info!(count = ids.len(), ?patch, "batch_update_metadata");
    let lib_patch = LibMetadataPatch {
        title: patch.title,
        tags: patch.tags,
        tags_add: patch.tags_add,
        tags_remove: patch.tags_remove,
    };
    let mut ok = Vec::with_capacity(ids.len());
    for id in ids {
        match search::apply_metadata_patch_by_gid(&services.pool, id, &lib_patch) {
            Ok(()) => {
                let _ = app_handle.emit(IMAGE_UPDATED_EVENT, id);
                ok.push(id);
            }
            Err(e) => tracing::warn!(id, error = %e, "batch: apply failed"),
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

    // Group by folder so each folder's DB is touched only once.
    let grouped = search::group_gids_by_folder(&ids);
    let services_clone = services.inner().clone();

    let out = tauri::async_runtime::spawn_blocking(move || {
        let mut deleted_gids: Vec<i64> = Vec::new();
        let mut failed: Vec<DeleteFailure> = Vec::new();

        for (folder_id, local_ids) in grouped {
            // Resolve folder root.
            let root = match services_clone.pool.folder(folder_id) {
                Ok(f) => PathBuf::from(f.path),
                Err(e) => {
                    tracing::warn!(folder_id, error = %e, "delete: folder lookup failed");
                    for l in local_ids {
                        failed.push(DeleteFailure {
                            id: db::pack_global_id(folder_id, l),
                            path: String::new(),
                            error: format!("{e}"),
                        });
                    }
                    continue;
                }
            };
            // Get rel_paths.
            let entries = match services_clone.pool.library(folder_id).and_then(|lib| {
                let conn = lib.lock()?;
                library::get_rel_paths(&conn, &local_ids)
            }) {
                Ok(v) => v,
                Err(e) => {
                    for l in local_ids {
                        failed.push(DeleteFailure {
                            id: db::pack_global_id(folder_id, l),
                            path: String::new(),
                            error: format!("{e}"),
                        });
                    }
                    continue;
                }
            };
            // Trash / delete files.
            let mut succeeded_local: Vec<i64> = Vec::new();
            for (local_id, rel) in entries {
                let abs = root.join(&rel);
                match delete_one(&abs, permanent) {
                    Ok(()) => {
                        let gid = db::pack_global_id(folder_id, local_id);
                        thumbnail::delete_thumbnails(
                            &services_clone.thumb_cache_dir,
                            gid,
                        );
                        succeeded_local.push(local_id);
                        deleted_gids.push(gid);
                    }
                    Err(e) => failed.push(DeleteFailure {
                        id: db::pack_global_id(folder_id, local_id),
                        path: abs.to_string_lossy().to_string(),
                        error: e,
                    }),
                }
            }
            // Remove DB rows for successfully deleted files.
            if !succeeded_local.is_empty() {
                if let Ok(lib) = services_clone.pool.library(folder_id) {
                    if let Ok(mut conn) = lib.lock() {
                        let _ = library::delete_images(&mut conn, &succeeded_local);
                    }
                }
            }
        }

        DeleteResult {
            deleted: deleted_gids,
            failed,
        }
    })
    .await
    .map_err(|e| AppError::Internal(format!("delete join: {e}")))?;

    if !out.deleted.is_empty() {
        let _ = app_handle.emit(IMAGES_DELETED_EVENT, &out.deleted);
    }
    Ok(out)
}

fn delete_one(path: &Path, permanent: bool) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }
    if permanent {
        std::fs::remove_file(path).map_err(|e| e.to_string())
    } else {
        trash::delete(path).map_err(|e| e.to_string())
    }
}
