//! Real image classifier backed by OpenAI's CLIP-ViT-B/32.
//!
//! Zero-shot image tagging: we run the CLIP vision encoder on a
//! thumbnail to get a 512-dimensional embedding, then compare it by
//! cosine similarity against a set of pre-computed text embeddings —
//! one for each tag in our curated vocabulary (see
//! [`super::model_manager::VOCAB_TEXT`]). The top-K tags above a
//! confidence threshold are returned.
//!
//! Two model heads are involved:
//!
//! 1. **Vision encoder** — small, hot path. Runs once per image
//!    thumbnail. The CLIP model is kept resident in memory for the
//!    lifetime of the classifier instance.
//! 2. **Text encoder** — used exactly once, at first-run, to build
//!    the vocabulary embedding cache under
//!    `<app_data_dir>/models/clip/photo_vocab_v1.embeddings.f32`.
//!    Subsequent Magpie launches reuse the cache.
//!
//! Backend: [`candle`](https://github.com/huggingface/candle),
//! HuggingFace's pure-Rust ML framework. We deliberately avoid ONNX
//! Runtime here — the pyke.io CDN that hosts `ort-sys`'s prebuilt
//! binaries has been unreliable, and candle removes that whole
//! failure mode by being 100% Rust. Runs on the CPU only; a modern
//! machine takes ~150-300 ms per image, which is fine for the
//! folder-add flow.

use crate::error::{AppError, AppResult};
use crate::types::TagSuggestion;
use candle_core::{DType, Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::clip::{self, ClipModel};
use image::imageops::FilterType;
use std::path::Path;
use std::sync::Mutex;
use tokenizers::Tokenizer;

use super::classifier::ImageClassifier;
use super::model_manager;

/// CLIP's fixed input size (ViT-B/32 patch size × 7 patches per side).
const CLIP_IMAGE_SIZE: u32 = 224;

/// Per-channel mean applied to normalized [0,1] pixel values before
/// the vision encoder. These are the CLIP-specific constants baked
/// into every OpenAI CLIP checkpoint.
const CLIP_MEAN: [f32; 3] = [0.48145466, 0.4578275, 0.40821073];
const CLIP_STD: [f32; 3] = [0.26862954, 0.26130258, 0.27577711];

/// Length of CLIP's fixed context window (BPE tokens).
const CLIP_CONTEXT_LEN: usize = 77;

/// Prompt template — CLIP works best on natural-language prompts,
/// so we wrap every vocab entry once at pre-compute time.
const PROMPT_TEMPLATE: &str = "a photo of a ";

/// Minimum cosine similarity, in the raw [-1, 1] range, that a
/// suggestion must reach to be surfaced to the user. Tuned on a
/// small photo corpus — anything much lower produces junk tags.
const MIN_COSINE: f32 = 0.20;

/// Ceiling on how many tags we volunteer per image; higher counts
/// tend to bring in progressively noisier suggestions.
const MAX_TAGS_PER_IMAGE: usize = 6;

pub struct ClipClassifier {
    /// The whole CLIP model (vision + text + projection heads).
    /// `run` needs `&mut self` on some candle paths, so we wrap the
    /// model in a `Mutex` and expose only `&self` to the trait.
    model: Mutex<ClipModel>,
    /// L2-normalised text embeddings for the entire vocabulary,
    /// shape = `[vocab.len(), 512]` in row-major order.
    text_embeds: Vec<f32>,
    /// Vocabulary in the same order as `text_embeds`. `text_embeds`
    /// row `i` is the embedding for `vocab[i]`.
    vocab: Vec<String>,
    /// Cached embedding dimension read out of the vision model's
    /// output shape. All CLIP-ViT-B/32 outputs are 512.
    embed_dim: usize,
    /// Kept alive to service the `text_embeds` layout above.
    device: Device,
}

impl ClipClassifier {
    /// Build a classifier from files under `<app_data_dir>/models/clip/`.
    /// Requires that [`model_manager::check_status`] returns
    /// `ready() == true`; callers should use this in contexts where
    /// the model is expected to be present.
    pub fn try_load(app_data_dir: &Path) -> AppResult<Self> {
        let status = model_manager::check_status(app_data_dir)?;
        if !status.model_present || !status.tokenizer_present {
            return Err(AppError::Internal(
                "CLIP model files are not downloaded yet".into(),
            ));
        }

        let device = Device::Cpu;
        let model_path = model_manager::model_file_path(app_data_dir)?;
        let config = clip::ClipConfig::vit_base_patch32();

        // Load model weights (memory-mapped safetensors).
        let vb = unsafe {
            VarBuilder::from_mmaped_safetensors(&[&model_path], DType::F32, &device)
                .map_err(|e| AppError::Internal(format!("mmap safetensors: {e}")))?
        };
        let model = ClipModel::new(vb, &config)
            .map_err(|e| AppError::Internal(format!("build CLIP model: {e}")))?;

        // Build / load text embeddings — must be done AFTER the model
        // is available because the pre-compute step reuses the same
        // model to run the text encoder over every vocab entry.
        let vocab = model_manager::load_vocab();
        let text_embeds = ensure_text_embeddings(app_data_dir, &vocab, &model, &device)?;

        if text_embeds.is_empty() || text_embeds.len() % vocab.len() != 0 {
            return Err(AppError::Internal(format!(
                "vocab/embedding size mismatch: {} entries, {} floats",
                vocab.len(),
                text_embeds.len()
            )));
        }
        let embed_dim = text_embeds.len() / vocab.len();

        tracing::info!(
            vocab_size = vocab.len(),
            embed_dim,
            "CLIP classifier ready"
        );

        Ok(Self {
            model: Mutex::new(model),
            text_embeds,
            vocab,
            embed_dim,
            device,
        })
    }
}

impl ImageClassifier for ClipClassifier {
    fn classify(&self, image_bytes: &[u8]) -> AppResult<Vec<TagSuggestion>> {
        let pixel_tensor = preprocess_image(image_bytes, &self.device)?;

        let img_embed = {
            let model = self
                .model
                .lock()
                .map_err(|_| AppError::Internal("clip model mutex poisoned".into()))?;
            let features = model
                .get_image_features(&pixel_tensor)
                .map_err(|e| AppError::Internal(format!("vision inference: {e}")))?;
            let normalized = l2_normalize_tensor(&features)
                .map_err(|e| AppError::Internal(format!("l2 normalize image: {e}")))?;
            normalized
                .flatten_all()
                .and_then(|t| t.to_vec1::<f32>())
                .map_err(|e| AppError::Internal(format!("image embed to vec: {e}")))?
        };

        // Cosine similarity against every row of the cached text
        // matrix. Both sides are L2-normalised so the dot product
        // IS the cosine similarity.
        let mut scored: Vec<(f32, usize)> = Vec::with_capacity(self.vocab.len());
        for (i, row) in self.text_embeds.chunks_exact(self.embed_dim).enumerate() {
            let mut dot = 0.0f32;
            for k in 0..self.embed_dim {
                dot += img_embed[k] * row[k];
            }
            scored.push((dot, i));
        }
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        let mut out = Vec::with_capacity(MAX_TAGS_PER_IMAGE);
        for (score, idx) in scored.into_iter().take(MAX_TAGS_PER_IMAGE * 2) {
            if score < MIN_COSINE {
                break;
            }
            out.push(TagSuggestion {
                name: self.vocab[idx].clone(),
                confidence: score,
            });
            if out.len() >= MAX_TAGS_PER_IMAGE {
                break;
            }
        }
        Ok(out)
    }

    fn min_confidence(&self) -> f32 {
        MIN_COSINE
    }

    fn max_tags_per_image(&self) -> usize {
        MAX_TAGS_PER_IMAGE
    }
}

/// Decode arbitrary image bytes into a CLIP-normalised tensor of
/// shape `[1, 3, 224, 224]`. Aspect ratio is preserved via a
/// centre-crop after resizing the short side to 224 px, mirroring
/// the reference OpenAI preprocessing (`CenterCrop(224)`).
fn preprocess_image(bytes: &[u8], device: &Device) -> AppResult<Tensor> {
    let img = image::load_from_memory(bytes)
        .map_err(|e| AppError::Internal(format!("decode image: {e}")))?;

    // Resize so the shorter side becomes exactly 224 px.
    let (w, h) = (img.width(), img.height());
    let scale = CLIP_IMAGE_SIZE as f32 / w.min(h) as f32;
    let new_w = ((w as f32 * scale).round() as u32).max(CLIP_IMAGE_SIZE);
    let new_h = ((h as f32 * scale).round() as u32).max(CLIP_IMAGE_SIZE);
    let resized = img.resize_exact(new_w, new_h, FilterType::CatmullRom);

    // Centre-crop to 224×224.
    let x0 = (new_w - CLIP_IMAGE_SIZE) / 2;
    let y0 = (new_h - CLIP_IMAGE_SIZE) / 2;
    let cropped = image::imageops::crop_imm(&resized, x0, y0, CLIP_IMAGE_SIZE, CLIP_IMAGE_SIZE)
        .to_image();
    let rgb = image::DynamicImage::ImageRgba8(cropped).to_rgb8();

    let n_px = (CLIP_IMAGE_SIZE * CLIP_IMAGE_SIZE) as usize;
    let mut out = vec![0f32; 3 * n_px];
    for (i, pixel) in rgb.pixels().enumerate() {
        // NCHW layout: channel-major, so channel `c` at pixel `i`
        // sits at `c*n_px + i`.
        let r = pixel.0[0] as f32 / 255.0;
        let g = pixel.0[1] as f32 / 255.0;
        let b = pixel.0[2] as f32 / 255.0;
        out[i] = (r - CLIP_MEAN[0]) / CLIP_STD[0];
        out[n_px + i] = (g - CLIP_MEAN[1]) / CLIP_STD[1];
        out[2 * n_px + i] = (b - CLIP_MEAN[2]) / CLIP_STD[2];
    }

    Tensor::from_vec(
        out,
        (
            1usize,
            3usize,
            CLIP_IMAGE_SIZE as usize,
            CLIP_IMAGE_SIZE as usize,
        ),
        device,
    )
    .map_err(|e| AppError::Internal(format!("build pixel tensor: {e}")))
}

/// Row-wise L2 normalisation on a `(batch, dim)` tensor. Same
/// operation as `nn.functional.normalize(x, p=2, dim=-1)` in PyTorch.
fn l2_normalize_tensor(t: &Tensor) -> candle_core::Result<Tensor> {
    let sqr = t.sqr()?;
    let sum = sqr.sum_keepdim(candle_core::D::Minus1)?;
    let norm = sum.sqrt()?;
    t.broadcast_div(&norm)
}

/// Load or compute the vocabulary text embeddings. Cached on disk so
/// we only pay the cost once per (Magpie install, vocab version)
/// pair.
fn ensure_text_embeddings(
    app_data_dir: &Path,
    vocab: &[String],
    model: &ClipModel,
    device: &Device,
) -> AppResult<Vec<f32>> {
    let embeds_p = model_manager::embeddings_path(app_data_dir)?;
    let sha_p = model_manager::embeddings_vocab_sha_path(app_data_dir)?;
    let want_sha = model_manager::current_vocab_sha256();

    let cache_valid = embeds_p.is_file()
        && sha_p.is_file()
        && std::fs::read_to_string(&sha_p)
            .map(|s| s.trim() == want_sha)
            .unwrap_or(false);

    if cache_valid {
        tracing::debug!("reusing cached text embeddings");
        let bytes = std::fs::read(&embeds_p)
            .map_err(|e| AppError::Internal(format!("read text embeds cache: {e}")))?;
        let floats: &[f32] = bytemuck::cast_slice(&bytes);
        return Ok(floats.to_vec());
    }

    tracing::info!(vocab_size = vocab.len(), "building text embeddings cache");
    let tokenizer_path = model_manager::tokenizer_path(app_data_dir)?;
    let embeds = compute_text_embeddings(&tokenizer_path, vocab, model, device)?;

    let bytes: &[u8] = bytemuck::cast_slice(&embeds);
    std::fs::write(&embeds_p, bytes)
        .map_err(|e| AppError::Internal(format!("write text embeds cache: {e}")))?;
    std::fs::write(&sha_p, want_sha)
        .map_err(|e| AppError::Internal(format!("write vocab sha marker: {e}")))?;

    tracing::info!(bytes_written = bytes.len(), "text embeddings cache built");
    Ok(embeds)
}

/// Run the CLIP text encoder over every vocab entry and return a
/// flat `[vocab.len(), embed_dim]` row-major matrix of L2-normalised
/// embeddings.
fn compute_text_embeddings(
    tokenizer_path: &Path,
    vocab: &[String],
    model: &ClipModel,
    device: &Device,
) -> AppResult<Vec<f32>> {
    let tokenizer = Tokenizer::from_file(tokenizer_path)
        .map_err(|e| AppError::Internal(format!("load tokenizer: {e}")))?;

    let pad_id = *tokenizer
        .get_vocab(true)
        .get("<|endoftext|>")
        .ok_or_else(|| {
            AppError::Internal("tokenizer missing <|endoftext|> pad token".into())
        })?;

    let mut out: Vec<f32> = Vec::with_capacity(vocab.len() * 512);

    for entry in vocab {
        let prompt = format!("{PROMPT_TEMPLATE}{entry}");
        let enc = tokenizer
            .encode(prompt.as_str(), true)
            .map_err(|e| AppError::Internal(format!("tokenize \"{prompt}\": {e}")))?;

        // Pad / truncate to the fixed 77-token context.
        let mut tokens: Vec<u32> = enc.get_ids().to_vec();
        if tokens.len() > CLIP_CONTEXT_LEN {
            tokens.truncate(CLIP_CONTEXT_LEN);
        } else {
            while tokens.len() < CLIP_CONTEXT_LEN {
                tokens.push(pad_id);
            }
        }

        let input_ids = Tensor::new(tokens.as_slice(), device)
            .and_then(|t| t.unsqueeze(0))
            .map_err(|e| AppError::Internal(format!("build id tensor: {e}")))?;

        let features = model
            .get_text_features(&input_ids)
            .map_err(|e| AppError::Internal(format!("text inference: {e}")))?;
        let normalized = l2_normalize_tensor(&features)
            .map_err(|e| AppError::Internal(format!("l2 normalize text: {e}")))?;

        let vec: Vec<f32> = normalized
            .flatten_all()
            .and_then(|t| t.to_vec1::<f32>())
            .map_err(|e| AppError::Internal(format!("text embed to vec: {e}")))?;

        out.extend(vec);
    }

    Ok(out)
}
