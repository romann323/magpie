//! Local file store for the CLIP model.
//!
//! The model files are **not** bundled with Magpie — they add
//! ~600 MB and change independently of the app's binary. Instead
//! we lazily download them from HuggingFace on first use and cache
//! them under `<app_data_dir>/models/clip/`.
//!
//! We pin every model file by SHA-256 (the value HuggingFace
//! advertises as the LFS `oid`) so a corrupt or man-in-the-middle
//! download is caught before candle ever opens it. The tokenizer
//! JSON is treated as trusted (~2 MB, plain-text) and not
//! checksum-pinned; it's overwritten on any content mismatch during
//! a re-download.
//!
//! On-disk layout (relative to `<app_data_dir>/models/clip/`):
//!
//! ```text
//! model.safetensors              (~605 MB, OpenAI CLIP-ViT-B/32)
//! tokenizer.json                 (~2 MB, CLIP BPE tokenizer)
//! photo_vocab_v1.embeddings.f32  (derived, ~N*512*4 bytes)
//! photo_vocab_v1.vocab.sha256    (derived, SHA-256 of the vocab file
//!                                 the embeddings above were computed
//!                                 against — used to invalidate the
//!                                 cache when we ship a new vocab)
//! ```

use crate::error::{AppError, AppResult};
use futures_util::StreamExt;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter};

pub const AI_MODEL_DOWNLOAD_EVENT: &str = "app://ai-model-download";

/// Subdirectory under `app_data_dir` where all AI model files live.
pub const MODEL_SUBDIR: &str = "models/clip";

/// Bundled tag vocabulary — embedded at compile time so a first-run
/// app can always compute text embeddings once the model has been
/// downloaded (assuming the model + tokenizer are already cached).
pub const VOCAB_TEXT: &str = include_str!("resources/photo_vocab_v1.txt");

/// Bump when the vocab file changes semantically. Included in the
/// text-embedding cache key so old caches are invalidated cleanly.
pub const VOCAB_VERSION: &str = "v1";

/// Descriptor for a downloadable model artefact.
struct ModelFile {
    /// Filename on disk, under `models/clip/`.
    filename: &'static str,
    /// Full HTTPS URL to fetch from.
    url: &'static str,
    /// Expected size in bytes; used only for the progress bar total.
    expected_size: u64,
    /// SHA-256 of the whole file; verified after download. Set to
    /// `None` for files we don't checksum (tokenizer JSON is trusted
    /// because it's small and human-readable).
    sha256: Option<&'static str>,
}

/// The two files that must be present before the CLIP classifier
/// can be initialised.
///
/// The safetensors file lives on `refs/pr/15` of the OpenAI CLIP
/// repository — that PR added the safetensors variant to what would
/// otherwise be a PyTorch-only checkpoint. HuggingFace URL-encodes
/// the `refs/pr/15` revision as `refs%2Fpr%2F15`.
const REQUIRED_FILES: &[ModelFile] = &[
    ModelFile {
        filename: "model.safetensors",
        url: "https://huggingface.co/openai/clip-vit-base-patch32/resolve/refs%2Fpr%2F15/model.safetensors",
        expected_size: 605_157_884,
        sha256: Some("99d28a652e6ec46629ab7047a0ac82c69b1fe11e0ce672c43af65d3a9a3fc05d"),
    },
    ModelFile {
        filename: "tokenizer.json",
        url: "https://huggingface.co/openai/clip-vit-base-patch32/resolve/main/tokenizer.json",
        expected_size: 2_224_041,
        sha256: None,
    },
];

/// Sum of `expected_size` across every required file. Used by the
/// UI's total-bytes indicator before a download starts.
pub const TOTAL_DOWNLOAD_BYTES: u64 = 605_157_884 + 2_224_041;

/// Status snapshot emitted from `check_ai_model_status`. Mirrored by
/// [`crate::types::AiModelStatus`].
#[derive(Clone, Debug, Serialize)]
pub struct ModelStatus {
    pub model_dir: PathBuf,
    pub model_present: bool,
    pub tokenizer_present: bool,
    pub embeddings_present: bool,
    pub total_bytes: u64,
    pub bytes_on_disk: u64,
}

impl ModelStatus {
    /// True iff every required file is present and the embeddings
    /// cache has been built. When this is false the CLIP classifier
    /// cannot start.
    pub fn ready(&self) -> bool {
        self.model_present && self.tokenizer_present && self.embeddings_present
    }
}

/// Absolute path to the CLIP model cache directory. Created if it
/// doesn't exist.
pub fn model_dir(app_data_dir: &Path) -> AppResult<PathBuf> {
    let dir = app_data_dir.join(MODEL_SUBDIR);
    if !dir.exists() {
        std::fs::create_dir_all(&dir)
            .map_err(|e| AppError::Internal(format!("create clip model dir: {e}")))?;
    }
    Ok(dir)
}

/// Path to the CLIP model weights file (safetensors).
pub fn model_file_path(app_data_dir: &Path) -> AppResult<PathBuf> {
    Ok(model_dir(app_data_dir)?.join("model.safetensors"))
}

/// Path to the tokenizer JSON file.
pub fn tokenizer_path(app_data_dir: &Path) -> AppResult<PathBuf> {
    Ok(model_dir(app_data_dir)?.join("tokenizer.json"))
}

/// Path to the pre-computed vocab text embeddings.
pub fn embeddings_path(app_data_dir: &Path) -> AppResult<PathBuf> {
    Ok(model_dir(app_data_dir)?
        .join(format!("photo_vocab_{VOCAB_VERSION}.embeddings.f32")))
}

/// Path to the file recording the SHA-256 of the vocab the cached
/// embeddings were computed against.
pub fn embeddings_vocab_sha_path(app_data_dir: &Path) -> AppResult<PathBuf> {
    Ok(model_dir(app_data_dir)?
        .join(format!("photo_vocab_{VOCAB_VERSION}.vocab.sha256")))
}

/// Compute a stable hash of the current vocabulary text so we can
/// detect when a Magpie upgrade shipped a new vocab and the cached
/// embeddings need to be re-computed.
pub fn current_vocab_sha256() -> String {
    let mut h = Sha256::new();
    h.update(VOCAB_TEXT.as_bytes());
    hex::encode(h.finalize())
}

/// Non-blocking status probe. Every field is derived from filesystem
/// existence + size, no downloads or hashes are performed here.
pub fn check_status(app_data_dir: &Path) -> AppResult<ModelStatus> {
    let dir = model_dir(app_data_dir)?;
    let (model_present, model_bytes) = file_present(&dir, REQUIRED_FILES[0].filename);
    let (tokenizer_present, tokenizer_bytes) = file_present(&dir, REQUIRED_FILES[1].filename);

    let embeds_p = embeddings_path(app_data_dir)?;
    let sha_p = embeddings_vocab_sha_path(app_data_dir)?;
    let embeddings_present = embeds_p.is_file()
        && sha_p.is_file()
        && std::fs::read_to_string(&sha_p)
            .map(|s| s.trim() == current_vocab_sha256())
            .unwrap_or(false);

    Ok(ModelStatus {
        model_dir: dir,
        model_present,
        tokenizer_present,
        embeddings_present,
        total_bytes: TOTAL_DOWNLOAD_BYTES,
        bytes_on_disk: model_bytes + tokenizer_bytes,
    })
}

fn file_present(dir: &Path, name: &str) -> (bool, u64) {
    let p = dir.join(name);
    if let Ok(meta) = std::fs::metadata(&p) {
        if meta.is_file() {
            return (true, meta.len());
        }
    }
    (false, 0)
}

/// Progress payload emitted on [`AI_MODEL_DOWNLOAD_EVENT`] while a
/// download is in flight.
#[derive(Clone, Debug, Serialize)]
pub struct DownloadProgress {
    /// Which of the required files is currently being fetched.
    pub current_file: String,
    /// Bytes received for the current file so far.
    pub current_bytes: u64,
    /// Total expected bytes for the current file (from HuggingFace).
    pub current_total: u64,
    /// Cumulative bytes received across all files so far.
    pub total_bytes: u64,
    /// Cumulative expected bytes across all files.
    pub total_expected: u64,
    /// True on the final "done" event. `error` is populated if the
    /// download failed.
    pub finished: bool,
    /// Error message on failure, otherwise `None`.
    pub error: Option<String>,
}

/// Download every missing required file. Idempotent — files that
/// already exist and pass checksum verification are skipped.
///
/// Emits [`AI_MODEL_DOWNLOAD_EVENT`] progress events. Returns once
/// all files are on disk and verified.
///
/// This does **not** build the text-embedding cache (that's the CLIP
/// classifier's responsibility, since it needs a live model
/// session).
pub async fn ensure_downloaded(
    app_data_dir: PathBuf,
    app_handle: AppHandle,
) -> AppResult<()> {
    let dir = model_dir(&app_data_dir)?;

    let client = reqwest::Client::builder()
        .user_agent(concat!("Magpie/", env!("CARGO_PKG_VERSION")))
        // HuggingFace CDN sometimes stalls for a few seconds on the
        // first chunk; be generous on the initial connect timeout.
        .connect_timeout(std::time::Duration::from_secs(30))
        // Overall per-request cap. 30 min is enough for a 600 MB file
        // over a slow-ish connection; we retry the whole request if
        // it trips this timeout.
        .timeout(std::time::Duration::from_secs(30 * 60))
        // Honour the redirect from huggingface.co to the CDN
        // (`us.aws.cdn.hf.co`) and any signed-URL bounces after that.
        .redirect(reqwest::redirect::Policy::limited(10))
        // TCP keep-alive keeps NAT / stateful firewalls from silently
        // dropping the connection during long downloads.
        .tcp_keepalive(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| AppError::Internal(format!("build http client: {}", chain(&e))))?;

    let total_expected = TOTAL_DOWNLOAD_BYTES;
    let mut cumulative_bytes: u64 = 0;

    for entry in REQUIRED_FILES {
        let target = dir.join(entry.filename);

        // Skip if the file is already present and its checksum
        // matches. Files without a pinned checksum (tokenizer.json)
        // are re-downloaded whenever missing but trusted when present.
        if target.is_file() {
            match entry.sha256 {
                Some(expected) if file_sha256(&target).ok().as_deref() == Some(expected) => {
                    cumulative_bytes += entry.expected_size;
                    tracing::debug!(file = entry.filename, "already present, checksum ok");
                    let _ = emit_progress(&app_handle, &DownloadProgress {
                        current_file: entry.filename.to_string(),
                        current_bytes: entry.expected_size,
                        current_total: entry.expected_size,
                        total_bytes: cumulative_bytes,
                        total_expected,
                        finished: false,
                        error: None,
                    });
                    continue;
                }
                None => {
                    cumulative_bytes += target.metadata().map(|m| m.len()).unwrap_or(0);
                    tracing::debug!(file = entry.filename, "already present (unchecked)");
                    continue;
                }
                _ => {
                    tracing::warn!(file = entry.filename, "present but checksum mismatch; re-downloading");
                    let _ = std::fs::remove_file(&target);
                }
            }
        }

        let file_bytes = download_one_with_retry(
            &client,
            entry,
            &dir,
            &app_handle,
            cumulative_bytes,
            total_expected,
        )
        .await?;

        cumulative_bytes += file_bytes;
        tracing::info!(file = entry.filename, "downloaded ok");
    }

    let _ = emit_progress(&app_handle, &DownloadProgress {
        current_file: String::new(),
        current_bytes: cumulative_bytes,
        current_total: cumulative_bytes,
        total_bytes: cumulative_bytes,
        total_expected,
        finished: true,
        error: None,
    });

    Ok(())
}

fn emit_progress(app: &AppHandle, p: &DownloadProgress) -> tauri::Result<()> {
    app.emit(AI_MODEL_DOWNLOAD_EVENT, p)
}

/// Format a full `Error → source → source → …` chain into a single
/// human-readable line. reqwest's `Display` impl only prints the
/// top-level message ("error sending request for url (…)"), hiding
/// whether the real cause was a TLS handshake failure, a DNS lookup,
/// a proxy issue, or a mid-stream read timeout — which is exactly
/// what a support user needs to see.
fn chain(err: &dyn std::error::Error) -> String {
    let mut msg = err.to_string();
    let mut cur = err.source();
    while let Some(e) = cur {
        msg.push_str(" | caused by: ");
        msg.push_str(&e.to_string());
        cur = e.source();
    }
    msg
}

/// Number of times we retry a full download of one file before
/// giving up. The Range-header resume path means a mid-stream drop
/// costs at most one round-trip, not a full re-download.
const MAX_DOWNLOAD_ATTEMPTS: u32 = 4;

/// Wraps [`download_one`] with retry-with-backoff. Each retry
/// preserves the `.part` file so `download_one` can resume from
/// wherever the previous attempt left off via `Range: bytes=N-`.
async fn download_one_with_retry(
    client: &reqwest::Client,
    entry: &ModelFile,
    dir: &Path,
    app_handle: &AppHandle,
    cumulative_bytes_before_file: u64,
    total_expected: u64,
) -> AppResult<u64> {
    let mut attempt = 0u32;
    let mut last_err: Option<String> = None;
    while attempt < MAX_DOWNLOAD_ATTEMPTS {
        attempt += 1;
        match download_one(
            client,
            entry,
            dir,
            app_handle,
            cumulative_bytes_before_file,
            total_expected,
        )
        .await
        {
            Ok(bytes) => return Ok(bytes),
            Err(e) => {
                let msg = format!("{e}");
                tracing::warn!(
                    file = entry.filename,
                    attempt,
                    error = %msg,
                    "download failed"
                );
                last_err = Some(msg);
                if attempt >= MAX_DOWNLOAD_ATTEMPTS {
                    break;
                }
                // Exponential backoff: 2s, 4s, 8s
                let secs = 2u64.pow(attempt);
                tokio::time::sleep(std::time::Duration::from_secs(secs)).await;
            }
        }
    }
    Err(AppError::Internal(format!(
        "download of {} failed after {} attempts: {}",
        entry.filename,
        MAX_DOWNLOAD_ATTEMPTS,
        last_err.unwrap_or_else(|| "no error captured".to_string()),
    )))
}

/// Download a single model file, resuming from an existing `.part`
/// if present. Streams bytes with a full progress event every
/// ~200 ms and verifies the SHA-256 of the whole file (existing
/// bytes + newly-downloaded bytes) before promoting `.part` to its
/// final name.
async fn download_one(
    client: &reqwest::Client,
    entry: &ModelFile,
    dir: &Path,
    app_handle: &AppHandle,
    cumulative_bytes_before_file: u64,
    total_expected: u64,
) -> AppResult<u64> {
    use std::io::{Read, Write};

    let part = dir.join(format!("{}.part", entry.filename));

    // Resume path: if a `.part` already exists, hash whatever bytes
    // are there so the final checksum still covers the whole file,
    // then open the file for append so new chunks land at the end.
    let mut hasher = Sha256::new();
    let mut bytes_so_far: u64 = 0;
    let mut file = if part.is_file() {
        let mut existing = std::fs::File::open(&part)
            .map_err(|e| AppError::Internal(format!("open .part for resume: {e}")))?;
        let mut buf = [0u8; 8192];
        loop {
            let n = existing.read(&mut buf).map_err(|e| {
                AppError::Internal(format!("read .part for resume: {e}"))
            })?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
            bytes_so_far += n as u64;
        }
        drop(existing);
        std::fs::OpenOptions::new()
            .append(true)
            .open(&part)
            .map_err(|e| AppError::Internal(format!("reopen .part append: {e}")))?
    } else {
        std::fs::File::create(&part)
            .map_err(|e| AppError::Internal(format!("create {}: {e}", part.display())))?
    };

    // If the .part is already the full expected size (or bigger),
    // a previous pass was interrupted right before verification —
    // skip the request and fall through to the checksum step.
    let already_complete = bytes_so_far >= entry.expected_size;

    tracing::info!(
        file = entry.filename,
        url = entry.url,
        resume_from = bytes_so_far,
        already_complete,
        "downloading"
    );

    if !already_complete {
        let mut req = client.get(entry.url);
        if bytes_so_far > 0 {
            req = req.header(reqwest::header::RANGE, format!("bytes={bytes_so_far}-"));
        }
        let resp = req.send().await.map_err(|e| {
            AppError::Internal(format!("send GET {}: {}", entry.filename, chain(&e)))
        })?;
        let status = resp.status();
        if !status.is_success() {
            return Err(AppError::Internal(format!(
                "http {} for {} (url: {})",
                status, entry.filename, entry.url,
            )));
        }
        let content_length_total = resp
            .content_length()
            .map(|c| c + bytes_so_far)
            .unwrap_or(entry.expected_size);

        let mut stream = resp.bytes_stream();
        let mut last_emit = std::time::Instant::now();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| {
                AppError::Internal(format!(
                    "stream chunk for {}: {}",
                    entry.filename,
                    chain(&e)
                ))
            })?;
            hasher.update(&chunk);
            file.write_all(&chunk)
                .map_err(|e| AppError::Internal(format!("write chunk: {e}")))?;
            bytes_so_far += chunk.len() as u64;

            if last_emit.elapsed() >= std::time::Duration::from_millis(200) {
                last_emit = std::time::Instant::now();
                let _ = emit_progress(app_handle, &DownloadProgress {
                    current_file: entry.filename.to_string(),
                    current_bytes: bytes_so_far,
                    current_total: content_length_total,
                    total_bytes: cumulative_bytes_before_file + bytes_so_far,
                    total_expected,
                    finished: false,
                    error: None,
                });
            }
        }
    }

    drop(file);

    // Verify checksum if we have one. On mismatch we wipe the .part
    // so the next attempt starts fresh instead of resuming corrupt
    // bytes.
    if let Some(expected) = entry.sha256 {
        let actual = hex::encode(hasher.finalize());
        if actual != expected {
            let _ = std::fs::remove_file(&part);
            return Err(AppError::Internal(format!(
                "checksum mismatch for {}: expected {expected}, got {actual}",
                entry.filename
            )));
        }
    }

    let target = dir.join(entry.filename);
    std::fs::rename(&part, &target)
        .map_err(|e| AppError::Internal(format!("promote {}: {e}", entry.filename)))?;

    Ok(bytes_so_far)
}

/// SHA-256 of a file, computed in a single pass. Used both to verify
/// freshly-downloaded artefacts and to spot-check cached ones on
/// startup.
fn file_sha256(p: &Path) -> AppResult<String> {
    use std::io::Read;
    let mut f = std::fs::File::open(p)
        .map_err(|e| AppError::Internal(format!("open {}: {e}", p.display())))?;
    let mut h = Sha256::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = f
            .read(&mut buf)
            .map_err(|e| AppError::Internal(format!("read {}: {e}", p.display())))?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(hex::encode(h.finalize()))
}

/// Load the bundled vocabulary as a `Vec` of trimmed, unique,
/// non-comment entries. Order is preserved — that's the order the
/// pre-computed embeddings blob will be stored in.
pub fn load_vocab() -> Vec<String> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for raw in VOCAB_TEXT.lines() {
        let t = raw.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        let lower = t.to_ascii_lowercase();
        if seen.insert(lower.clone()) {
            out.push(lower);
        }
    }
    out
}
