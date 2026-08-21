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
    queries::list_tags(&services.db, prefix.as_deref())
}

#[tauri::command]
pub async fn rename_tag(
    services: State<'_, Arc<AppServices>>,
    old_name: String,
    new_name: String,
) -> AppResult<()> {
    queries::rename_tag(&services.db, &old_name, &new_name)
}

#[tauri::command]
pub async fn delete_tag(
    services: State<'_, Arc<AppServices>>,
    name: String,
) -> AppResult<()> {
    queries::delete_tag(&services.db, &name)
}
