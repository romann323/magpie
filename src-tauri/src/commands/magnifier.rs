//! Commands used by the Magnifier popup window.
//!
//! The main window hands off a `MagnifierContext` (which image to
//! show + the filter/sort of the list to walk) before opening the
//! popup; the popup reads that context on mount via
//! [`get_magnifier_context`] and updates the "current image" pointer
//! as the user navigates via [`set_magnifier_current`].

use crate::core::AppServices;
use crate::error::AppResult;
use crate::types::{MagnifierContext, ImageFilter, ImageSort};
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub async fn get_magnifier_context(
    services: State<'_, Arc<AppServices>>,
) -> AppResult<MagnifierContext> {
    services.magnifier_context()
}

#[tauri::command]
pub async fn set_magnifier_context(
    services: State<'_, Arc<AppServices>>,
    image_id: Option<i64>,
    filter: Option<ImageFilter>,
    sort: Option<ImageSort>,
) -> AppResult<()> {
    services.set_magnifier_context(MagnifierContext {
        image_id,
        filter: filter.unwrap_or_default(),
        sort: sort.unwrap_or_default(),
    })
}

#[tauri::command]
pub async fn set_magnifier_current(
    services: State<'_, Arc<AppServices>>,
    image_id: Option<i64>,
) -> AppResult<()> {
    services.set_magnifier_current(image_id)
}
