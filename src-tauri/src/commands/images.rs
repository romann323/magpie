use crate::core::metadata::read as meta_read;
use crate::core::{thumbnail, AppServices};
use crate::db::queries::{self, MetadataPatch as DbMetadataPatch};
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
    services
        .db()?
        .with_conn(|conn| queries::query_images(conn, &filter, &sort, &page))
}

#[tauri::command]
pub async fn get_image(
    services: State<'_, Arc<AppServices>>,
    id: i64,
) -> AppResult<ImageDetails> {
    let db = services.db()?;
    let (mut row, root) = db
        .with_conn(|conn| queries::get_image_with_root(conn, id))?
        .ok_or(AppError::ImageNotFound(id))?;

    // Re-read metadata from disk if the file has changed since last import.
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
                        db.with_conn_mut(|conn| queries::set_image_meta(conn, id, &fresh))?;
                        if let Some(refreshed) = db.with_conn(|conn| queries::get_image_row(conn, id))? {
                            row = refreshed;
                        }
                    }
                }
            }
        }
    }

    Ok(enrich_details(services.inner(), root, row))
}

fn enrich_details(
    services: &Arc<AppServices>,
    root: PathBuf,
    row: queries::ImageRow,
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
            id: row.id,
            folder_id: row.folder_id,
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
        user_tags: row.user_tags,
        auto_tags: row.auto_tags,
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
    let db_patch = DbMetadataPatch {
        title: patch.title.clone(),
        tags: patch.tags.clone(),
        tags_add: patch.tags_add.clone(),
        tags_remove: patch.tags_remove.clone(),
    };
    let db = services.db()?;
    db.with_conn_mut(|conn| queries::apply_metadata_patch(conn, id, &db_patch))?;
    let _ = app_handle.emit(IMAGE_UPDATED_EVENT, id);
    let (row, root) = db
        .with_conn(|conn| queries::get_image_with_root(conn, id))?
        .ok_or(AppError::ImageNotFound(id))?;
    Ok(enrich_details(services.inner(), root, row))
}

#[tauri::command]
pub async fn batch_update_metadata(
    services: State<'_, Arc<AppServices>>,
    app_handle: AppHandle,
    ids: Vec<i64>,
    patch: MetadataPatch,
) -> AppResult<Vec<i64>> {
    tracing::info!(count = ids.len(), ?patch, "batch_update_metadata");
    let db_patch = DbMetadataPatch {
        title: patch.title,
        tags: patch.tags,
        tags_add: patch.tags_add,
        tags_remove: patch.tags_remove,
    };
    let db = services.db()?;
    let mut ok = Vec::with_capacity(ids.len());
    for id in ids {
        match db.with_conn_mut(|conn| queries::apply_metadata_patch(conn, id, &db_patch)) {
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

/// Rename the file on disk and update the DB in the same call. On
/// success returns the refreshed `ImageDetails`. Fails without touching
/// disk when the new name is invalid, would land in another folder, or
/// collides with an existing sibling.
#[tauri::command]
pub async fn rename_image(
    services: State<'_, Arc<AppServices>>,
    app_handle: AppHandle,
    id: i64,
    new_filename: String,
) -> AppResult<ImageDetails> {
    tracing::info!(id, new_filename = %new_filename, "rename_image");
    let trimmed = new_filename.trim().to_string();
    validate_filename(&trimmed)?;

    let db = services.db()?;
    let (row, root) = db
        .with_conn(|conn| queries::get_image_with_root(conn, id))?
        .ok_or(AppError::ImageNotFound(id))?;

    if row.filename == trimmed {
        // No-op: return the current record so the frontend sees a
        // successful save even when nothing changed.
        return Ok(enrich_details(services.inner(), root, row));
    }

    let old_abs = root.join(&row.rel_path);
    let parent_abs = old_abs
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| root.clone());
    let new_abs = parent_abs.join(&trimmed);

    if new_abs.exists() {
        return Err(AppError::BadInput(format!(
            "a file already exists at {}",
            new_abs.display()
        )));
    }

    std::fs::rename(&old_abs, &new_abs)
        .map_err(|e| AppError::Internal(format!("rename on disk failed: {e}")))?;

    if let Err(e) = db.with_conn_mut(|conn| queries::rename_image_row(conn, id, &trimmed)) {
        // Try to roll the FS rename back if the DB update failed.
        if let Err(e2) = std::fs::rename(&new_abs, &old_abs) {
            tracing::error!(
                error = %e,
                rollback_error = %e2,
                "rename DB failed AND rollback failed; DB and disk are out of sync"
            );
        }
        return Err(e);
    }

    let _ = app_handle.emit(IMAGE_UPDATED_EVENT, id);
    let (row, root) = db
        .with_conn(|conn| queries::get_image_with_root(conn, id))?
        .ok_or(AppError::ImageNotFound(id))?;
    Ok(enrich_details(services.inner(), root, row))
}

fn validate_filename(name: &str) -> AppResult<()> {
    if name.is_empty() {
        return Err(AppError::BadInput("filename is empty".into()));
    }
    if name.contains('/')
        || name.contains('\\')
        || name.contains(':')
        || name.contains('*')
        || name.contains('?')
        || name.contains('"')
        || name.contains('<')
        || name.contains('>')
        || name.contains('|')
    {
        return Err(AppError::BadInput(
            "filename contains an unsupported character".into(),
        ));
    }
    if name == "." || name == ".." {
        return Err(AppError::BadInput("filename cannot be . or ..".into()));
    }
    Ok(())
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
    let services_clone = services.inner().clone();

    let out = tauri::async_runtime::spawn_blocking(move || -> AppResult<DeleteResult> {
        let db = services_clone.db()?;
        let mut deleted: Vec<i64> = Vec::new();
        let mut failed: Vec<DeleteFailure> = Vec::new();

        // Resolve (id, folder_id, rel_path) up front.
        let entries = db.with_conn(|conn| queries::get_paths(conn, &ids))?;

        // Cache folder root lookups.
        let mut roots: std::collections::HashMap<i64, PathBuf> =
            std::collections::HashMap::new();

        let mut succeeded: Vec<i64> = Vec::new();
        for (id, folder_id, rel) in entries {
            let root = match roots.get(&folder_id).cloned() {
                Some(r) => r,
                None => {
                    let f = db.with_conn(|conn| queries::get_folder(conn, folder_id))?;
                    let r = PathBuf::from(f.path);
                    roots.insert(folder_id, r.clone());
                    r
                }
            };
            let abs = root.join(&rel);
            match delete_one(&abs, permanent) {
                Ok(()) => {
                    if let Ok(cache_dir) = services_clone.thumb_cache_dir() {
                        thumbnail::delete_thumbnails(&cache_dir, id);
                    }
                    succeeded.push(id);
                    deleted.push(id);
                }
                Err(e) => failed.push(DeleteFailure {
                    id,
                    path: abs.to_string_lossy().to_string(),
                    error: e,
                }),
            }
        }

        if !succeeded.is_empty() {
            let _ = db.with_conn_mut(|conn| queries::delete_images(conn, &succeeded));
        }

        Ok(DeleteResult { deleted, failed })
    })
    .await
    .map_err(|e| AppError::Internal(format!("delete join: {e}")))??;

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
