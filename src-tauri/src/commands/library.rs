use crate::core::scanner;
use crate::core::AppServices;
use crate::db::queries;
use crate::error::{PicOrgError, PicOrgResult};
use crate::types::{LibraryFolder, ScanResult};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, State};

#[tauri::command]
pub async fn add_library_folder(
    services: State<'_, Arc<AppServices>>,
    app_handle: AppHandle,
    path: String,
) -> PicOrgResult<LibraryFolder> {
    let canon = std::fs::canonicalize(&path).map_err(|_| PicOrgError::PathNotFound(path.clone()))?;
    if !canon.is_dir() {
        return Err(PicOrgError::NotADirectory(canon.display().to_string()));
    }
    let canon_str = canon.to_string_lossy().to_string();
    let folder = queries::add_folder(&services.db, &canon_str)?;

    // Kick off background scan
    let services_bg = services.inner().clone();
    let app_handle_bg = app_handle.clone();
    let folder_id = folder.id;
    let path_bg = PathBuf::from(&canon_str);
    tauri::async_runtime::spawn(async move {
        if let Err(e) = scanner::scan_folder(services_bg, app_handle_bg, folder_id, path_bg).await {
            tracing::error!(error = %e, "scan failed");
        }
    });

    Ok(folder)
}

#[tauri::command]
pub async fn remove_library_folder(
    services: State<'_, Arc<AppServices>>,
    id: i64,
) -> PicOrgResult<()> {
    queries::remove_folder(&services.db, id)
}

#[tauri::command]
pub async fn list_library_folders(
    services: State<'_, Arc<AppServices>>,
) -> PicOrgResult<Vec<LibraryFolder>> {
    queries::list_folders(&services.db)
}

#[tauri::command]
pub async fn rescan_folder(
    services: State<'_, Arc<AppServices>>,
    app_handle: AppHandle,
    id: i64,
) -> PicOrgResult<ScanResult> {
    let folder = queries::list_folders(&services.db)?
        .into_iter()
        .find(|f| f.id == id)
        .ok_or(PicOrgError::FolderNotFound(id))?;
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
) -> PicOrgResult<Vec<ScanResult>> {
    let folders = queries::list_folders(&services.db)?;
    let mut out = Vec::new();
    for f in folders {
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
