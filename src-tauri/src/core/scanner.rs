use crate::core::metadata::read as meta_read;
use crate::core::thumbnail;
use crate::core::{is_image_ext, AppServices};
use crate::db::queries;
use crate::db::queries::FileStat;
use crate::error::{PicOrgError, PicOrgResult};
use crate::types::{ScanProgress, ScanResult};
use jwalk::WalkDir;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};

pub const SCAN_EVENT: &str = "picorg://scan";

#[derive(Default)]
struct Counters {
    added: AtomicI64,
    updated: AtomicI64,
    errors: AtomicI64,
    processed: AtomicI64,
}

pub async fn scan_folder(
    services: Arc<AppServices>,
    app_handle: AppHandle,
    folder_id: i64,
    root: PathBuf,
) -> PicOrgResult<ScanResult> {
    if !root.exists() {
        return Err(PicOrgError::PathNotFound(root.display().to_string()));
    }
    if !root.is_dir() {
        return Err(PicOrgError::NotADirectory(root.display().to_string()));
    }

    tracing::info!(?root, folder_id, "starting scan");

    // ---------- Phase 1: enumerate files (fast) ----------
    let files: Vec<PathBuf> = tokio::task::spawn_blocking({
        let root = root.clone();
        move || collect_image_paths(&root)
    })
    .await
    .map_err(|e| PicOrgError::Internal(format!("scan task join: {e}")))?;

    let total = files.len() as i64;
    tracing::info!(count = total, "scan enumerated files");

    let _ = emit_progress(
        &app_handle,
        &ScanProgress {
            folder_id,
            processed: 0,
            total,
            current_path: None,
            finished: false,
        },
    );

    // ---------- Phase 2: process each file ----------
    let counters = Arc::new(Counters::default());
    let seen: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::with_capacity(files.len())));

    let cpus = num_cpus::get().max(2);
    let semaphore = Arc::new(tokio::sync::Semaphore::new(cpus));

    let mut handles = Vec::with_capacity(files.len());
    for path in files {
        let permit = semaphore.clone().acquire_owned().await.unwrap();
        let services = services.clone();
        let app_handle = app_handle.clone();
        let counters = counters.clone();
        let seen = seen.clone();

        let handle = tokio::task::spawn_blocking(move || {
            let path_str = path.to_string_lossy().to_string();
            {
                let mut s = seen.lock().unwrap();
                s.insert(path_str.clone());
            }

            match process_one(&services, folder_id, &path) {
                Ok(ProcessOutcome::Added) => {
                    counters.added.fetch_add(1, Ordering::Relaxed);
                }
                Ok(ProcessOutcome::Updated) => {
                    counters.updated.fetch_add(1, Ordering::Relaxed);
                }
                Ok(ProcessOutcome::Unchanged) => {}
                Err(e) => {
                    tracing::warn!(?path, error = %e, "scan process failed");
                    counters.errors.fetch_add(1, Ordering::Relaxed);
                }
            }
            let n = counters.processed.fetch_add(1, Ordering::Relaxed) + 1;
            if n == total || n % 25 == 0 {
                let _ = emit_progress(
                    &app_handle,
                    &ScanProgress {
                        folder_id,
                        processed: n,
                        total,
                        current_path: Some(path_str),
                        finished: false,
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

    // ---------- Phase 3: mark missing ----------
    let seen_set = Arc::try_unwrap(seen)
        .map(|m| m.into_inner().unwrap())
        .unwrap_or_else(|arc| arc.lock().unwrap().clone());
    let removed = queries::mark_folder_paths_missing(&services.db, folder_id, &seen_set)?;

    let result = ScanResult {
        folder_id,
        added: counters.added.load(Ordering::Relaxed),
        updated: counters.updated.load(Ordering::Relaxed),
        removed,
        errors: counters.errors.load(Ordering::Relaxed),
    };

    let now = chrono::Utc::now().timestamp_millis();
    let _ = queries::set_last_scan_at(&services.db, folder_id, now);

    let _ = emit_progress(
        &app_handle,
        &ScanProgress {
            folder_id,
            processed: total,
            total,
            current_path: None,
            finished: true,
        },
    );

    tracing::info!(?result, "scan finished");
    Ok(result)
}

fn emit_progress(app: &AppHandle, p: &ScanProgress) -> tauri::Result<()> {
    app.emit(SCAN_EVENT, p)
}

fn collect_image_paths(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for entry in WalkDir::new(root)
        .skip_hidden(true)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_file() {
            let p = entry.path();
            if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
                if is_image_ext(ext) {
                    out.push(p);
                }
            }
        }
    }
    out
}

enum ProcessOutcome {
    Added,
    Updated,
    Unchanged,
}

fn process_one(
    services: &Arc<AppServices>,
    folder_id: i64,
    path: &Path,
) -> PicOrgResult<ProcessOutcome> {
    let meta_fs = std::fs::metadata(path)?;
    let size_bytes = meta_fs.len() as i64;
    let mtime_ms = meta_fs
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);

    let filename = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    let stat = FileStat {
        folder_id,
        path: path.to_string_lossy().to_string(),
        filename,
        ext,
        size_bytes,
        mtime_ms,
    };

    // was_new: did we insert a fresh row?
    let existed_before = queries::image_exists_by_path(&services.db, &stat.path)?;
    let (image_id, changed) = queries::upsert_image_stat(&services.db, &stat)?;

    if !changed {
        return Ok(ProcessOutcome::Unchanged);
    }

    match meta_read::read_all(path) {
        Ok(meta) => {
            queries::set_image_meta(&services.db, image_id, &meta)?;
        }
        Err(e) => {
            tracing::debug!(?path, error = %e, "metadata read failed");
        }
    }

    if let Err(e) = thumbnail::ensure_thumbnails(&services.thumb_cache_dir, path, image_id) {
        tracing::debug!(?path, error = %e, "thumbnail gen failed");
    }

    if existed_before {
        Ok(ProcessOutcome::Updated)
    } else {
        Ok(ProcessOutcome::Added)
    }
}
