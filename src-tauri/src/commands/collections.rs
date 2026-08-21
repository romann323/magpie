use crate::core::AppServices;
use crate::db::queries;
use crate::error::AppResult;
use crate::types::{ImageFilter, SmartCollection};
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub async fn list_smart_collections(
    services: State<'_, Arc<AppServices>>,
) -> AppResult<Vec<SmartCollection>> {
    queries::list_smart_collections(&services.db)
}

#[tauri::command]
pub async fn create_smart_collection(
    services: State<'_, Arc<AppServices>>,
    name: String,
    filter: ImageFilter,
) -> AppResult<SmartCollection> {
    queries::create_smart_collection(&services.db, &name, &filter)
}

#[tauri::command]
pub async fn delete_smart_collection(
    services: State<'_, Arc<AppServices>>,
    id: i64,
) -> AppResult<()> {
    queries::delete_smart_collection(&services.db, id)
}
