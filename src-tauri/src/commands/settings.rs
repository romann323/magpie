use crate::core::project::{AppSettings, FontSize, Theme};
use crate::core::AppServices;
use crate::error::AppResult;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::State;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettingsPatch {
    #[serde(default)]
    pub theme: Option<Theme>,
    #[serde(default)]
    pub font_size: Option<FontSize>,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub ai_auto_tag: Option<bool>,
}

#[tauri::command]
pub async fn get_app_settings(
    services: State<'_, Arc<AppServices>>,
) -> AppResult<AppSettings> {
    services.get_settings()
}

#[tauri::command]
pub async fn update_app_settings(
    services: State<'_, Arc<AppServices>>,
    patch: AppSettingsPatch,
) -> AppResult<AppSettings> {
    services.update_settings(|s| {
        if let Some(t) = patch.theme {
            s.theme = t;
        }
        if let Some(fs) = patch.font_size {
            s.font_size = fs;
        }
        if let Some(l) = patch.language {
            s.language = l;
        }
        if let Some(v) = patch.ai_auto_tag {
            s.ai_auto_tag = v;
        }
    })
}
