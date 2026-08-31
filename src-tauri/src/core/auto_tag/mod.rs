//! Automatic AI tagging pipeline.
//!
//! When the user has **Settings → Auto-tag photos** enabled, this
//! module runs after the filesystem scanner finishes for a newly-added
//! library folder. It walks every non-missing image in the folder,
//! feeds a thumbnail into an [`ImageClassifier`], applies the
//! suggestions as ordinary user-side tags via
//! [`crate::db::queries::apply_metadata_patch`], and remembers a
//! per-image fingerprint so a rerun on the same, unchanged file is
//! cheap.
//!
//! Concurrency:
//! - Per-image work runs on a bounded semaphore (one permit per CPU),
//!   mirroring [`crate::core::scanner`].
//! - The whole `tag_folder` invocation acquires
//!   [`AppServices::auto_tag_gate`], so if the user drops several
//!   folders on the app in quick succession the AI passes queue up
//!   FIFO instead of thrashing CPU/GPU in parallel.
//!
//! Events:
//! - Emits [`AUTO_TAG_EVENT`] (`app://auto-tag`) with an
//!   [`AutoTagProgress`] payload — the status bar renders this
//!   alongside the scan progress line.

pub mod classifier;
pub mod clip_classifier;
pub mod model_manager;

use crate::core::thumbnail;
use crate::core::AppServices;
use crate::db::queries::{self, AutoTagCandidate};
use crate::error::{AppError, AppResult};
use crate::types::{AutoTagProgress, AutoTagResult};
use classifier::ImageClassifier;
use clip_classifier::ClipClassifier;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};

pub const AUTO_TAG_EVENT: &str = "app://auto-tag";

/// Default entry point invoked from `commands::library::add_library_folder`
/// after the scanner finishes.
///
/// Looks for a downloaded CLIP model under `<app_data_dir>/models/clip/`.
/// When present, uses [`ClipClassifier`] (zero-shot ranking against
/// our curated vocabulary). When absent, emits a single "finished
/// with error" progress event and returns immediately — the UI's
/// Auto-tag toggle is responsible for prompting the download; we
/// don't silently downgrade to the mock any more.
pub async fn tag_folder(
    services: Arc<AppServices>,
    app_handle: AppHandle,
    folder_id: i64,
) -> AppResult<AutoTagResult> {
    let status = model_manager::check_status(&services.app_data_dir)?;
    if !status.model_present || !status.tokenizer_present {
        tracing::warn!(
            model = status.model_present,
            tokenizer = status.tokenizer_present,
            "auto-tag skipped: model files not fully downloaded"
        );
        let _ = emit_progress(
            &app_handle,
            &AutoTagProgress {
                folder_id,
                processed: 0,
                total: 0,
                current_path: None,
                tags_added: 0,
                skipped: 0,
                finished: true,
                error: Some(
                    "AI model not downloaded — open Settings → Auto-tag photos to install it."
                        .into(),
                ),
            },
        );
        return Ok(AutoTagResult {
            folder_id,
            ..Default::default()
        });
    }

    // Building the CLIP classifier can be slow (loads ~90 MB of
    // weights, builds the DirectML session, and — on very first
    // launch — runs the text encoder over the vocab). Do it on a
    // blocking task so the async runtime keeps spinning.
    let app_data_dir = services.app_data_dir.clone();
    let build_result = tokio::task::spawn_blocking(move || ClipClassifier::try_load(&app_data_dir))
        .await
        .map_err(|e| AppError::Internal(format!("classifier build join: {e}")))?;
    let classifier: Arc<dyn ImageClassifier> = match build_result {
        Ok(c) => Arc::new(c),
        Err(e) => {
            tracing::error!(error = %e, "failed to initialise CLIP classifier");
            let _ = emit_progress(
                &app_handle,
                &AutoTagProgress {
                    folder_id,
                    processed: 0,
                    total: 0,
                    current_path: None,
                    tags_added: 0,
                    skipped: 0,
                    finished: true,
                    error: Some(format!("AI classifier failed to start: {e}")),
                },
            );
            return Err(e);
        }
    };

    tag_folder_with(services, app_handle, folder_id, classifier).await
}

/// Same as [`tag_folder`] but with an injected classifier. Tests use
/// this to feed a deterministic stub without pulling `MockClassifier`
/// off the module directly.
pub async fn tag_folder_with(
    services: Arc<AppServices>,
    app_handle: AppHandle,
    folder_id: i64,
    classifier: Arc<dyn ImageClassifier>,
) -> AppResult<AutoTagResult> {
    // Serialize AI passes across folders — see module docs.
    let _guard = services.auto_tag_gate.lock().await;

    let db = services.db()?;
    let folder = db.with_conn(|conn| queries::get_folder(conn, folder_id))?;
    let root = PathBuf::from(&folder.path);

    let candidates: Vec<AutoTagCandidate> =
        db.with_conn(|conn| queries::list_auto_tag_candidates(conn, folder_id))?;
    let total = candidates.len() as i64;

    tracing::info!(folder_id, total, "auto-tag pass starting");

    let cache_dir = services.thumb_cache_dir()?;
    let processed = Arc::new(AtomicI64::new(0));
    let skipped = Arc::new(AtomicI64::new(0));
    let tagged_images = Arc::new(AtomicI64::new(0));
    let tags_added = Arc::new(AtomicI64::new(0));
    let errors = Arc::new(AtomicI64::new(0));

    let _ = emit_progress(
        &app_handle,
        &AutoTagProgress {
            folder_id,
            processed: 0,
            total,
            current_path: None,
            tags_added: 0,
            skipped: 0,
            finished: false,
            error: None,
        },
    );

    let cpus = num_cpus::get().max(2);
    let semaphore = Arc::new(tokio::sync::Semaphore::new(cpus));

    let mut handles = Vec::with_capacity(candidates.len());
    for cand in candidates {
        let permit = semaphore.clone().acquire_owned().await.unwrap();
        let services = services.clone();
        let app_handle = app_handle.clone();
        let processed = processed.clone();
        let skipped = skipped.clone();
        let tagged_images = tagged_images.clone();
        let tags_added = tags_added.clone();
        let errors = errors.clone();
        let cache_dir = cache_dir.clone();
        let root = root.clone();
        let classifier = classifier.clone();

        let handle = tokio::task::spawn_blocking(move || {
            let cand_path = root.join(&cand.rel_path);
            let cand_path_str = cand_path.to_string_lossy().to_string();

            let already =
                cand.ai_tag_hash.as_deref() == Some(cand.fingerprint.as_str());
            let outcome = if already {
                skipped.fetch_add(1, Ordering::Relaxed);
                Ok(0i64)
            } else {
                tag_one(
                    &services,
                    &cache_dir,
                    &cand_path,
                    &cand,
                    classifier.as_ref(),
                )
            };

            match outcome {
                Ok(n) if n > 0 => {
                    tagged_images.fetch_add(1, Ordering::Relaxed);
                    tags_added.fetch_add(n, Ordering::Relaxed);
                }
                Ok(_) => {}
                Err(e) => {
                    tracing::warn!(
                        image_id = cand.id,
                        error = %e,
                        "auto-tag: image failed"
                    );
                    errors.fetch_add(1, Ordering::Relaxed);
                }
            }

            let n = processed.fetch_add(1, Ordering::Relaxed) + 1;
            if n == total || n % 5 == 0 {
                let _ = emit_progress(
                    &app_handle,
                    &AutoTagProgress {
                        folder_id,
                        processed: n,
                        total,
                        current_path: Some(cand_path_str),
                        tags_added: tags_added.load(Ordering::Relaxed),
                        skipped: skipped.load(Ordering::Relaxed),
                        finished: false,
                        error: None,
                    },
                );
            }

            drop(permit);
        });
        handles.push(handle);
    }
    for h in handles {
        let _ = h.await;
    }

    let result = AutoTagResult {
        folder_id,
        processed: processed.load(Ordering::Relaxed),
        skipped: skipped.load(Ordering::Relaxed),
        tagged_images: tagged_images.load(Ordering::Relaxed),
        tags_added: tags_added.load(Ordering::Relaxed),
        errors: errors.load(Ordering::Relaxed),
    };

    let _ = emit_progress(
        &app_handle,
        &AutoTagProgress {
            folder_id,
            processed: total,
            total,
            current_path: None,
            tags_added: result.tags_added,
            skipped: result.skipped,
            finished: true,
            error: None,
        },
    );

    tracing::info!(?result, "auto-tag pass finished");
    Ok(result)
}

fn emit_progress(app: &AppHandle, p: &AutoTagProgress) -> tauri::Result<()> {
    app.emit(AUTO_TAG_EVENT, p)
}

/// Process one candidate. Returns the number of tags actually added
/// (may be 0 if every suggestion was already on the image).
fn tag_one(
    services: &Arc<AppServices>,
    cache_dir: &Path,
    src_path: &Path,
    cand: &AutoTagCandidate,
    classifier: &dyn ImageClassifier,
) -> AppResult<i64> {
    if !src_path.exists() {
        return Err(AppError::PathNotFound(src_path.display().to_string()));
    }

    // We prefer the small thumbnail — it's already cached from the
    // scan step and cheap to decode. If the format isn't previewable
    // (RAW/HEIC/video etc.) `ensure_thumbnails` is a no-op, so we bail
    // out gracefully rather than trying to decode a video with the
    // `image` crate.
    let _ = thumbnail::ensure_thumbnails(cache_dir, src_path, cand.id);
    let thumb_p =
        thumbnail::thumb_path(cache_dir, cand.id, crate::types::ThumbSize::Small);
    if !thumb_p.exists() {
        tracing::debug!(
            image_id = cand.id,
            ext = %cand.ext,
            "auto-tag: no thumbnail available; skipping"
        );
        return Ok(0);
    }
    let bytes = std::fs::read(&thumb_p)
        .map_err(|e| AppError::Internal(format!("read thumbnail: {e}")))?;

    let mut suggestions = classifier.classify(&bytes)?;
    suggestions.retain(|s| s.confidence >= classifier.min_confidence());
    suggestions.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    suggestions.truncate(classifier.max_tags_per_image());
    if suggestions.is_empty() {
        // Still mark as tagged so we don't reclassify every run.
        let db = services.db()?;
        let now = chrono::Utc::now().timestamp_millis();
        db.with_conn(|conn| queries::mark_image_ai_tagged(conn, cand.id, &cand.fingerprint, now))?;
        return Ok(0);
    }

    let names: Vec<String> = suggestions.into_iter().map(|s| s.name).collect();
    let added_count = names.len() as i64;

    let db = services.db()?;
    // Write into the `'auto'` source so the tags render under the
    // read-only "Automatic tags" section in the details panel, next
    // to any XMP-derived auto tags. Duplicates that already exist
    // in either source are a no-op; the row count stays sane.
    db.with_conn_mut(|conn| queries::add_auto_tags_for_image(conn, cand.id, &names))?;

    let now = chrono::Utc::now().timestamp_millis();
    db.with_conn(|conn| queries::mark_image_ai_tagged(conn, cand.id, &cand.fingerprint, now))?;

    Ok(added_count)
}
