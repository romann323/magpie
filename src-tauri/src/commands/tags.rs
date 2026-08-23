use crate::core::AppServices;
use crate::db::{library, search};
use crate::error::AppResult;
use crate::types::TagStats;
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub async fn list_tags(
    services: State<'_, Arc<AppServices>>,
    prefix: Option<String>,
) -> AppResult<Vec<TagStats>> {
    search::list_all_tags(&services.pool, prefix.as_deref())
}

/// Rename `old_name` → `new_name` in **every** registered folder. Merges
/// (drops old row, moves references) when `new_name` already exists.
#[tauri::command]
pub async fn rename_tag(
    services: State<'_, Arc<AppServices>>,
    old_name: String,
    new_name: String,
) -> AppResult<()> {
    let folders = services.pool.list_folders()?;
    for f in folders {
        if !f.is_available {
            continue;
        }
        let lib = services.pool.library(f.id)?;
        let mut conn = lib.lock()?;
        library::rename_tag(&mut conn, &old_name, &new_name)?;
    }
    Ok(())
}

/// Delete a tag from every registered folder.
#[tauri::command]
pub async fn delete_tag(
    services: State<'_, Arc<AppServices>>,
    name: String,
) -> AppResult<()> {
    let folders = services.pool.list_folders()?;
    for f in folders {
        if !f.is_available {
            continue;
        }
        let lib = services.pool.library(f.id)?;
        let mut conn = lib.lock()?;
        library::delete_tag(&mut conn, &name)?;
    }
    Ok(())
}
