//! Diagnostic bridge from the frontend into the Rust log file. Lets the
//! renderer surface user actions (button clicks, mutation dispatches, etc.)
//! into `app.log` so we can debug flows that never make it to a real
//! Tauri command.

use crate::error::AppResult;

#[tauri::command]
pub fn log_frontend(level: String, msg: String) -> AppResult<()> {
    match level.as_str() {
        "error" => tracing::error!(target: "frontend", "{msg}"),
        "warn" => tracing::warn!(target: "frontend", "{msg}"),
        "info" => tracing::info!(target: "frontend", "{msg}"),
        _ => tracing::debug!(target: "frontend", "{msg}"),
    }
    Ok(())
}
