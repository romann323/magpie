use crate::core::auto_tag;
use crate::core::scanner;
use crate::core::AppServices;
use crate::db::queries;
use crate::error::{AppError, AppResult};
use crate::types::{LibraryFolder, ScanResult};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, State};

fn row_to_ipc(
    services: &AppServices,
    row: queries::LibraryFolderRow,
) -> LibraryFolder {
    let image_count = if row.is_available {
        services
            .db()
            .and_then(|db| db.with_conn(|conn| queries::count_images_in_folder(conn, row.id)))
            .unwrap_or(0)
    } else {
        0
    };
    LibraryFolder {
        id: row.id,
        path: row.path,
        added_at: row.added_at,
        last_scan_at: row.last_scan_at,
        image_count,
        is_available: row.is_available,
    }
}

#[tauri::command]
pub async fn add_library_folder(
    services: State<'_, Arc<AppServices>>,
    app_handle: AppHandle,
    path: String,
) -> AppResult<LibraryFolder> {
    let canon = std::fs::canonicalize(&path).map_err(|_| AppError::PathNotFound(path.clone()))?;
    if !canon.is_dir() {
        return Err(AppError::NotADirectory(canon.display().to_string()));
    }
    // Store the friendly (non-verbatim) form so any downstream Windows
    // Shell API call (used for first-scan tag import) accepts the path.
    let canon = crate::core::formats::common::strip_windows_verbatim_prefix(&canon);
    let db = services.db()?;
    let row = db.with_conn(|conn| queries::insert_folder(conn, &canon))?;

    let folder = row_to_ipc(&services, row);

    let services_bg = services.inner().clone();
    let app_handle_bg = app_handle.clone();
    let folder_id = folder.id;
    let path_bg = PathBuf::from(&folder.path);
    tauri::async_runtime::spawn(async move {
        match scanner::scan_folder(
            services_bg.clone(),
            app_handle_bg.clone(),
            folder_id,
            path_bg,
        )
        .await
        {
            Ok(_) => {
                let ai_on = services_bg
                    .get_settings()
                    .map(|s| s.ai_auto_tag)
                    .unwrap_or(false);
                if ai_on {
                    if let Err(e) =
                        auto_tag::tag_folder(services_bg, app_handle_bg, folder_id).await
                    {
                        tracing::error!(error = %e, "auto-tag failed");
                    }
                }
            }
            Err(e) => {
                tracing::error!(error = %e, "scan failed");
            }
        }
    });

    Ok(folder)
}

#[tauri::command]
pub async fn remove_library_folder(
    services: State<'_, Arc<AppServices>>,
    id: i64,
) -> AppResult<()> {
    services
        .db()?
        .with_conn(|conn| queries::delete_folder(conn, id))
}

#[tauri::command]
pub async fn list_library_folders(
    services: State<'_, Arc<AppServices>>,
) -> AppResult<Vec<LibraryFolder>> {
    let db = services.db()?;
    let folders = db.with_conn(queries::list_folders)?;
    let mut out: Vec<LibraryFolder> = Vec::with_capacity(folders.len());
    for mut row in folders {
        let available = std::path::Path::new(&row.path).is_dir();
        if available != row.is_available {
            let _ = db.with_conn(|conn| queries::set_folder_availability(conn, row.id, available));
            row.is_available = available;
        }
        out.push(row_to_ipc(services.inner(), row));
    }
    Ok(out)
}

#[tauri::command]
pub async fn rescan_folder(
    services: State<'_, Arc<AppServices>>,
    app_handle: AppHandle,
    id: i64,
) -> AppResult<ScanResult> {
    let db = services.db()?;
    let folder = db.with_conn(|conn| queries::get_folder(conn, id))?;
    scanner::scan_folder(
        services.inner().clone(),
        app_handle,
        folder.id,
        PathBuf::from(folder.path),
    )
    .await
}

#[tauri::command]
pub async fn rescan_all(
    services: State<'_, Arc<AppServices>>,
    app_handle: AppHandle,
) -> AppResult<Vec<ScanResult>> {
    let db = services.db()?;
    let folders = db.with_conn(queries::list_folders)?;
    let mut out = Vec::new();
    for f in folders {
        if !f.is_available {
            continue;
        }
        match scanner::scan_folder(
            services.inner().clone(),
            app_handle.clone(),
            f.id,
            PathBuf::from(f.path),
        )
        .await
        {
            Ok(r) => out.push(r),
            Err(e) => tracing::warn!(error = %e, "rescan_all: folder failed"),
        }
    }
    Ok(out)
}
