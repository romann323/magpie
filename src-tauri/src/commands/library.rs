use crate::core::scanner;
use crate::core::AppServices;
use crate::db::pool::library_db_path_for;
use crate::db::registry;
use crate::error::{AppError, AppResult};
use crate::types::{LibraryFolder, ScanResult, SyncRiskWarning};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::{AppHandle, State};

fn row_to_ipc(
    services: &AppServices,
    row: registry::LibraryFolderRow,
) -> LibraryFolder {
    // Count images in the folder's library DB (available folders only).
    let image_count = if row.is_available {
        services
            .pool
            .library(row.id)
            .and_then(|lib| {
                let conn = lib.lock()?;
                conn.query_row("SELECT COUNT(*) FROM images WHERE missing = 0", [], |r| {
                    r.get::<_, i64>(0)
                })
                .map_err(Into::into)
            })
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
    let row = services.pool.add_folder(&canon)?;

    let folder = row_to_ipc(&services, row);

    // Kick off background scan.
    let services_bg = services.inner().clone();
    let app_handle_bg = app_handle.clone();
    let folder_id = folder.id;
    let path_bg = PathBuf::from(&folder.path);
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
) -> AppResult<()> {
    services.pool.remove_folder(id)
}

#[tauri::command]
pub async fn list_library_folders(
    services: State<'_, Arc<AppServices>>,
) -> AppResult<Vec<LibraryFolder>> {
    let folders = services.pool.list_folders()?;
    Ok(folders
        .into_iter()
        .map(|r| row_to_ipc(services.inner(), r))
        .collect())
}

#[tauri::command]
pub async fn rescan_folder(
    services: State<'_, Arc<AppServices>>,
    app_handle: AppHandle,
    id: i64,
) -> AppResult<ScanResult> {
    let folder = services.pool.folder(id)?;
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
    let folders = services.pool.list_folders()?;
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

/// Check whether the caller-supplied path lives in a location where two
/// different PCs could edit the same `.magpie/library.db` at once
/// (OneDrive, Dropbox, Google Drive, iCloud, or a network share).
/// Returns `None` when the location is safe; the frontend shows a
/// confirm dialog with the message when non-null.
#[tauri::command]
pub async fn check_folder_sync_risk(path: String) -> AppResult<Option<SyncRiskWarning>> {
    Ok(detect_sync_risk(Path::new(&path)))
}

fn detect_sync_risk(path: &Path) -> Option<SyncRiskWarning> {
    let s = path.to_string_lossy().replace('/', "\\");
    let s_lower = s.to_ascii_lowercase();

    // UNC / network share
    if s.starts_with("\\\\") && !s.starts_with(r"\\?\") {
        return Some(SyncRiskWarning {
            provider: "network share".into(),
            message: format!(
                "This folder lives on a network share ({}). If two PCs open the same folder in Magpie at the same time, tag edits from one may not be visible to the other until you rescan, and simultaneous writes can corrupt the library. It's safe to use as long as only one PC has the folder open at a time.",
                s
            ),
        });
    }

    let checks: &[(&str, &[&str])] = &[
        ("OneDrive", &["\\onedrive", "\\onedrive - "]),
        ("Dropbox", &["\\dropbox"]),
        ("Google Drive", &["\\google drive", "\\googledrive"]),
        ("iCloud Drive", &["\\icloud", "\\icloud drive", "\\icloud~"]),
        ("Box", &["\\box"]),
    ];
    for (provider, needles) in checks {
        if needles.iter().any(|n| s_lower.contains(n)) {
            return Some(SyncRiskWarning {
                provider: (*provider).to_string(),
                message: format!(
                    "This folder looks like it's synced by {provider}. Magpie stores tags in a small \".magpie\\library.db\" file inside the folder — if you open the same folder from another PC through {provider} and edit tags on both machines at the same time, edits may be lost or the library can be corrupted by the sync client. Using it from one PC at a time is fine."
                ),
            });
        }
    }
    None
}

/// Utility used by tests / diagnostics: derives the library DB path
/// for a folder without touching the DB.
#[allow(dead_code)]
pub fn library_db_path(folder: &Path) -> PathBuf {
    library_db_path_for(folder)
}

#[cfg(test)]
mod sync_risk_tests {
    use super::detect_sync_risk;
    use std::path::Path;

    #[test]
    fn onedrive_warns() {
        let w = detect_sync_risk(Path::new(
            r"C:\Users\alice\OneDrive - Contoso\Photos\2024",
        ))
        .unwrap();
        assert_eq!(w.provider, "OneDrive");
    }

    #[test]
    fn dropbox_warns() {
        let w = detect_sync_risk(Path::new(r"C:\Users\alice\Dropbox\Photos"))
            .unwrap();
        assert_eq!(w.provider, "Dropbox");
    }

    #[test]
    fn unc_warns() {
        let w = detect_sync_risk(Path::new(r"\\nas\photos\2024")).unwrap();
        assert_eq!(w.provider, "network share");
    }

    #[test]
    fn local_disk_is_safe() {
        assert!(detect_sync_risk(Path::new(r"C:\Photos\2024")).is_none());
        assert!(detect_sync_risk(Path::new(r"D:\Users\alice\Pictures")).is_none());
    }
}
