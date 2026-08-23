use crate::core::AppServices;
use crate::db::queries;
use crate::error::AppResult;
use crate::types::{ImageFilter, SmartCollection};
use std::sync::Arc;
use tauri::State;

fn row_to_ipc(r: queries::SmartCollectionRow) -> SmartCollection {
    let filter: ImageFilter = serde_json::from_str(&r.filter).unwrap_or_default();
    SmartCollection {
        id: r.id,
        name: r.name,
        filter,
        sort_order: r.sort_order,
    }
}

#[tauri::command]
pub async fn list_smart_collections(
    services: State<'_, Arc<AppServices>>,
) -> AppResult<Vec<SmartCollection>> {
    let rows = services.db.with_conn(queries::list_smart_collections)?;
    Ok(rows.into_iter().map(row_to_ipc).collect())
}

#[tauri::command]
pub async fn create_smart_collection(
    services: State<'_, Arc<AppServices>>,
    name: String,
    filter: ImageFilter,
) -> AppResult<SmartCollection> {
    let filter_json = serde_json::to_string(&filter).unwrap_or_else(|_| "{}".into());
    let id = services
        .db
        .with_conn(|conn| queries::create_smart_collection(conn, &name, &filter_json))?;
    Ok(SmartCollection {
        id,
        name: name.trim().to_string(),
        filter,
        sort_order: 0,
    })
}

#[tauri::command]
pub async fn delete_smart_collection(
    services: State<'_, Arc<AppServices>>,
    id: i64,
) -> AppResult<()> {
    services
        .db
        .with_conn(|conn| queries::delete_smart_collection(conn, id))
}
