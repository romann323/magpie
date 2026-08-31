//! Tauri commands that back the "Auto-tag photos" Settings dialog.
//!
//! Every command here is thin — the heavy lifting lives in
//! [`crate::core::auto_tag::model_manager`] (downloads + on-disk
//! status) and [`crate::core::auto_tag::clip_classifier`] (inference).
//! We only:
//!
//! - Adapt the internal `ModelStatus` struct into the camelCase
//!   `AiModelStatus` shape the frontend types (`src/types.ts`) speak.
//! - Spawn the downloader on a Tauri async task and let it emit
//!   progress events itself.

use crate::core::auto_tag::model_manager;
use crate::core::AppServices;
use crate::error::{AppError, AppResult};
use crate::types::AiModelStatus;
use std::sync::Arc;
use tauri::{AppHandle, State};

/// Synchronous status probe. Cheap — pure filesystem `stat` calls.
/// The Settings dialog polls this after every download-progress
/// tick to update the "Model ready" / "Not downloaded" banner.
#[tauri::command]
pub async fn check_ai_model_status(
    services: State<'_, Arc<AppServices>>,
) -> AppResult<AiModelStatus> {
    let s = model_manager::check_status(&services.app_data_dir)?;
    Ok(AiModelStatus {
        ready: s.ready(),
        model_present: s.model_present,
        tokenizer_present: s.tokenizer_present,
        embeddings_present: s.embeddings_present,
        total_bytes: s.total_bytes,
        bytes_on_disk: s.bytes_on_disk,
    })
}

/// Start (or resume) a download of every missing CLIP model file.
/// The call returns immediately; progress is streamed on the
/// `app://ai-model-download` event and the final "done" event has
/// `finished = true`.
///
/// Guarded by a single mutex on `AppServices` so double-clicking the
/// button in Settings can't spawn two concurrent downloads racing on
/// the same target files.
#[tauri::command]
pub async fn download_ai_model(
    services: State<'_, Arc<AppServices>>,
    app_handle: AppHandle,
) -> AppResult<()> {
    // Optimistic pre-check so we return a friendly error instead of
    // firing off a doomed download on machines without network.
    let status = model_manager::check_status(&services.app_data_dir)?;
    if status.model_present && status.tokenizer_present {
        tracing::debug!("download_ai_model called but all files are already present");
        return Ok(());
    }

    // Ensure only one download runs at a time — reuse the auto-tag
    // gate; a download and an auto-tag pass would fight for the
    // same files anyway.
    let gate = services.auto_tag_gate.clone();
    let app_data_dir = services.app_data_dir.clone();
    let app_handle_clone = app_handle.clone();

    tauri::async_runtime::spawn(async move {
        let _guard = gate.lock().await;
        if let Err(e) =
            model_manager::ensure_downloaded(app_data_dir, app_handle_clone.clone()).await
        {
            tracing::error!(error = %e, "ai model download failed");
            let _ = tauri::Emitter::emit(
                &app_handle_clone,
                model_manager::AI_MODEL_DOWNLOAD_EVENT,
                model_manager::DownloadProgress {
                    current_file: String::new(),
                    current_bytes: 0,
                    current_total: 0,
                    total_bytes: 0,
                    total_expected: model_manager::TOTAL_DOWNLOAD_BYTES,
                    finished: true,
                    error: Some(format!("{e}")),
                },
            );
        }
    });

    Ok(())
}
/// Remove all downloaded model files. Used by the "Reset AI model"
/// button in Settings (rarely — mostly for support and debugging).
#[tauri::command]
pub async fn clear_ai_model(
    services: State<'_, Arc<AppServices>>,
) -> AppResult<()> {
    let dir = model_manager::model_dir(&services.app_data_dir)?;
    if dir.exists() {
        std::fs::remove_dir_all(&dir)
            .map_err(|e| AppError::Internal(format!("clear model dir: {e}")))?;
    }
    Ok(())
}
