use crate::core::AppServices;
use crate::db::queries;
use crate::error::AppResult;
use crate::types::TagStats;
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub async fn list_tags(
    services: State<'_, Arc<AppServices>>,
    prefix: Option<String>,
) -> AppResult<Vec<TagStats>> {
    services
        .db()?
        .with_conn(|conn| queries::list_all_tags(conn, prefix.as_deref()))
}

/// Rename `old_name` → `new_name`. Merges (drops old row, moves
/// references) when `new_name` already exists.
#[tauri::command]
pub async fn rename_tag(
    services: State<'_, Arc<AppServices>>,
    old_name: String,
    new_name: String,
) -> AppResult<()> {
    services
        .db()?
        .with_conn_mut(|conn| queries::rename_tag(conn, &old_name, &new_name))
}

/// Delete a tag globally.
#[tauri::command]
pub async fn delete_tag(
    services: State<'_, Arc<AppServices>>,
    name: String,
) -> AppResult<()> {
    services
        .db()?
        .with_conn_mut(|conn| queries::delete_tag(conn, &name))
}
