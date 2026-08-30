//! Native menu bar and menu-event bridge.
//!
//! Menu items are identified by short string IDs (e.g. `"proj_new"`).
//! Every click is forwarded to the frontend as an `app://menu` event
//! whose payload is the item ID; the renderer decides what to do.
//!
//! Enable/disable state for context-sensitive items (Edit → Undo/Redo)
//! is toggled by the frontend via [`set_menu_item_enabled`].

use crate::error::{AppError, AppResult};
use tauri::menu::{Menu, MenuBuilder, MenuItemBuilder, SubmenuBuilder};
use tauri::{AppHandle, Runtime, State, Wry};

pub const MENU_EVENT: &str = "app://menu";

// Menu item IDs. Kept in one place so the frontend router can pattern-
// match reliably; the constants are also referenced from unit tests.
pub const ID_PROJECT_NEW: &str = "proj_new";
pub const ID_PROJECT_OPEN: &str = "proj_open";
pub const ID_PROJECT_SAVE: &str = "proj_save";
pub const ID_PROJECT_SAVE_AS: &str = "proj_save_as";
pub const ID_PROJECT_CLOSE: &str = "proj_close";
pub const ID_PROJECT_QUIT: &str = "proj_quit";

pub const ID_EDIT_UNDO: &str = "edit_undo";
pub const ID_EDIT_REDO: &str = "edit_redo";

pub const ID_VIEW_MAGNIFIER: &str = "view_magnifier";

pub const ID_SETTINGS_LANGUAGE: &str = "set_language";
pub const ID_SETTINGS_THEME: &str = "set_theme";
pub const ID_SETTINGS_FONT_SIZE: &str = "set_font_size";
pub const ID_SETTINGS_AI_AUTO_TAG: &str = "set_ai_auto_tag";

/// Base label for the AI auto-tag toggle. The frontend appends a
/// checkmark by calling [`set_menu_item_label`] once it knows the
/// current setting value.
pub const AI_AUTO_TAG_LABEL_OFF: &str = "&Auto-tag photos";
pub const AI_AUTO_TAG_LABEL_ON: &str = "&Auto-tag photos  ✓";

pub fn build_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let project = SubmenuBuilder::new(app, "&Project")
        .item(
            &MenuItemBuilder::with_id(ID_PROJECT_NEW, "&New Project...")
                .accelerator("CmdOrCtrl+N")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id(ID_PROJECT_OPEN, "&Open Project...")
                .accelerator("CmdOrCtrl+O")
                .build(app)?,
        )
        .separator()
        .item(
            &MenuItemBuilder::with_id(ID_PROJECT_SAVE, "&Save Project")
                .accelerator("CmdOrCtrl+S")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id(ID_PROJECT_SAVE_AS, "Save Project &As...")
                .accelerator("CmdOrCtrl+Shift+S")
                .build(app)?,
        )
        .separator()
        .item(&MenuItemBuilder::with_id(ID_PROJECT_CLOSE, "&Close Project").build(app)?)
        .separator()
        .item(
            &MenuItemBuilder::with_id(ID_PROJECT_QUIT, "E&xit")
                .accelerator("Alt+F4")
                .build(app)?,
        )
        .build()?;

    let edit = SubmenuBuilder::new(app, "&Edit")
        .item(
            &MenuItemBuilder::with_id(ID_EDIT_UNDO, "&Undo")
                .accelerator("CmdOrCtrl+Z")
                .enabled(false)
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id(ID_EDIT_REDO, "&Redo")
                .accelerator("CmdOrCtrl+Y")
                .enabled(false)
                .build(app)?,
        )
        .build()?;

    let view = SubmenuBuilder::new(app, "&View")
        .item(
            &MenuItemBuilder::with_id(ID_VIEW_MAGNIFIER, "&Magnifier")
                .accelerator("F11")
                .enabled(false)
                .build(app)?,
        )
        .build()?;

    let settings = SubmenuBuilder::new(app, "&Settings")
        .item(&MenuItemBuilder::with_id(ID_SETTINGS_LANGUAGE, "&Language...").build(app)?)
        .item(&MenuItemBuilder::with_id(ID_SETTINGS_THEME, "&Theme...").build(app)?)
        .item(&MenuItemBuilder::with_id(ID_SETTINGS_FONT_SIZE, "&Font size...").build(app)?)
        .separator()
        .item(
            &MenuItemBuilder::with_id(ID_SETTINGS_AI_AUTO_TAG, AI_AUTO_TAG_LABEL_OFF)
                .build(app)?,
        )
        .build()?;

    let menu = MenuBuilder::new(app)
        .item(&project)
        .item(&edit)
        .item(&view)
        .item(&settings)
        .build()?;

    Ok(menu)
}

/// Toggle a menu item's enabled state. Called from the frontend when
/// the selection changes (Edit → Undo/Redo) or when the undo/redo
/// history transitions between empty and non-empty.
#[tauri::command]
pub fn set_menu_item_enabled(
    _services: State<'_, std::sync::Arc<crate::core::AppServices>>,
    app_handle: AppHandle<Wry>,
    id: String,
    enabled: bool,
) -> AppResult<()> {
    let menu = app_handle
        .menu()
        .ok_or_else(|| AppError::Internal("no application menu bound".into()))?;
    if let Some(item) = menu.get(&id) {
        if let Some(mi) = item.as_menuitem() {
            mi.set_enabled(enabled)
                .map_err(|e| AppError::Internal(format!("set_enabled: {e}")))?;
        }
    }
    Ok(())
}

/// Change a menu item's visible label. Used by the frontend to reflect
/// stateful settings (currently: `Auto-tag photos ✓` / `Auto-tag photos`)
/// without needing a native checkmark widget, which is awkward to
/// build at menu-creation time when the settings aren't yet known.
#[tauri::command]
pub fn set_menu_item_label(
    _services: State<'_, std::sync::Arc<crate::core::AppServices>>,
    app_handle: AppHandle<Wry>,
    id: String,
    label: String,
) -> AppResult<()> {
    let menu = app_handle
        .menu()
        .ok_or_else(|| AppError::Internal("no application menu bound".into()))?;
    if let Some(item) = menu.get(&id) {
        if let Some(mi) = item.as_menuitem() {
            mi.set_text(&label)
                .map_err(|e| AppError::Internal(format!("set_text: {e}")))?;
        }
    }
    Ok(())
}
