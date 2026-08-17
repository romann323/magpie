use crate::core::AppServices;
use crate::db::queries;
use crate::error::PicOrgResult;
use crate::types::{ImageFilter, SmartCollection};
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub async fn list_smart_collections(
    services: State<'_, Arc<AppServices>>,
) -> PicOrgResult<Vec<SmartCollection>> {
    queries::list_smart_collections(&services.db)
}

#[tauri::command]
pub async fn create_smart_collection(
    services: State<'_, Arc<AppServices>>,
    name: String,
    filter: ImageFilter,
) -> PicOrgResult<SmartCollection> {
    queries::create_smart_collection(&services.db, &name, &filter)
}

#[tauri::command]
pub async fn delete_smart_collection(
    services: State<'_, Arc<AppServices>>,
    id: i64,
) -> PicOrgResult<()> {
    queries::delete_smart_collection(&services.db, id)
}
