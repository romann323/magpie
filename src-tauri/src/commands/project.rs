use crate::core::project::{self, ProjectInfo, PROJECT_EXT};
use crate::core::AppServices;
use crate::error::{AppError, AppResult};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::State;

fn normalise_project_path(raw: &str) -> PathBuf {
    let mut p = PathBuf::from(raw);
    // Auto-append .magpie if the user didn't type it.
    if p.extension().is_none() {
        p.set_extension(PROJECT_EXT);
    }
    p
}

#[tauri::command]
pub async fn current_project(
    services: State<'_, Arc<AppServices>>,
) -> AppResult<Option<ProjectInfo>> {
    services.current_project()
}

#[tauri::command]
pub async fn create_project(
    services: State<'_, Arc<AppServices>>,
    path: String,
) -> AppResult<ProjectInfo> {
    let p = normalise_project_path(&path);
    let (db, info) = project::create_project(&p)?;
    let out = services.set_project(Some((db, info.clone())))?;
    Ok(out.unwrap_or(info))
}

#[tauri::command]
pub async fn open_project(
    services: State<'_, Arc<AppServices>>,
    path: String,
) -> AppResult<ProjectInfo> {
    let p = normalise_project_path(&path);
    let (db, info) = project::open_project(&p)?;
    let out = services.set_project(Some((db, info.clone())))?;
    Ok(out.unwrap_or(info))
}

#[tauri::command]
pub async fn save_project_as(
    services: State<'_, Arc<AppServices>>,
    path: String,
) -> AppResult<ProjectInfo> {
    let dst = normalise_project_path(&path);
    let db = services.db()?;
    let (new_db, info) = project::save_project_as(&db, &dst)?;
    // Drop the source Db handle before swapping in the new one.
    drop(db);
    let out = services.set_project(Some((new_db, info.clone())))?;
    Ok(out.unwrap_or(info))
}

/// Explicit save is a no-op for us — SQLite writes on every mutation.
/// We still surface a command so the menu item can wire to something.
#[tauri::command]
pub async fn save_project(
    services: State<'_, Arc<AppServices>>,
) -> AppResult<ProjectInfo> {
    services
        .current_project()?
        .ok_or(AppError::NoProjectOpen)
}

#[tauri::command]
pub async fn close_project(
    services: State<'_, Arc<AppServices>>,
) -> AppResult<()> {
    services.set_project(None)?;
    Ok(())
}
