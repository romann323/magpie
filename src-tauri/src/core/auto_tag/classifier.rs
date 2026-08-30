//! Image classifier abstraction used by the automatic-AI-tagging
//! pipeline.
//!
//! The trait is deliberately small — `classify(bytes) -> Vec<TagSuggestion>` —
//! so a real ONNX/CLIP sidecar can swap into the same slot later
//! without touching the caller in `auto_tag::mod`. Phase 1 ships only
//! [`MockClassifier`], which returns a stable pair of tags derived
//! from a hash of the input bytes; that gives deterministic, testable
//! output without bundling an ML model.

use crate::error::AppResult;
use crate::types::TagSuggestion;

pub trait ImageClassifier: Send + Sync {
    /// Classify a single image. `image_bytes` is the raw contents of
    /// a thumbnail (or the source file) — implementations are free to
    /// decode it however they like.
    fn classify(&self, image_bytes: &[u8]) -> AppResult<Vec<TagSuggestion>>;

    /// Minimum confidence a suggestion must clear before it's applied.
    /// The pipeline filters the classifier's output on this value.
    fn min_confidence(&self) -> f32 {
        0.5
    }

    /// Hard cap on the number of tags the pipeline will attach to a
    /// single image, regardless of how many the classifier suggests.
    fn max_tags_per_image(&self) -> usize {
        3
    }
}

/// Deterministic placeholder classifier. Picks two tags out of a
/// small fixed vocabulary based on a stable hash of the input bytes.
/// Enough to prove the wiring end-to-end and write reproducible tests
/// against — real inference (CLIP, MobileNet, etc.) can subclass in a
/// later PR without disturbing the caller.
#[derive(Debug, Default)]
pub struct MockClassifier;

impl MockClassifier {
    pub fn new() -> Self {
        Self
    }
}

const VOCAB: &[&str] = &[
    "landscape",
    "portrait",
    "indoor",
    "outdoor",
    "day",
    "night",
    "nature",
    "city",
    "water",
    "food",
    "people",
    "animal",
];

impl ImageClassifier for MockClassifier {
    fn classify(&self, image_bytes: &[u8]) -> AppResult<Vec<TagSuggestion>> {
        // Deterministic 64-bit hash of the bytes. `DefaultHasher`
        // isn't cryptographic but it's stable within a single process
        // — and that's all we need for a mock.
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        image_bytes.hash(&mut h);
        let hash = h.finish();

        // Two tags spaced apart in the vocabulary so we never return
        // duplicates. Confidence is derived from the same hash so it
        // stays deterministic per input.
        let a_idx = (hash as usize) % VOCAB.len();
        let b_idx = ((hash >> 8) as usize) % VOCAB.len();
        let b_idx = if b_idx == a_idx {
            (a_idx + 1) % VOCAB.len()
        } else {
            b_idx
        };

        let conf_a = 0.6 + (((hash >> 16) & 0xff) as f32 / 255.0) * 0.4; // 0.6..1.0
        let conf_b = 0.5 + (((hash >> 24) & 0xff) as f32 / 255.0) * 0.4; // 0.5..0.9

        Ok(vec![
            TagSuggestion {
                name: VOCAB[a_idx].to_string(),
                confidence: conf_a,
            },
            TagSuggestion {
                name: VOCAB[b_idx].to_string(),
                confidence: conf_b,
            },
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_is_deterministic() {
        let c = MockClassifier::new();
        let a = c.classify(b"same-bytes").unwrap();
        let b = c.classify(b"same-bytes").unwrap();
        assert_eq!(a.len(), 2);
        assert_eq!(
            a.iter().map(|s| &s.name).collect::<Vec<_>>(),
            b.iter().map(|s| &s.name).collect::<Vec<_>>()
        );
    }

    #[test]
    fn mock_returns_distinct_tags() {
        let c = MockClassifier::new();
        let out = c.classify(b"whatever").unwrap();
        assert_eq!(out.len(), 2);
        assert_ne!(out[0].name, out[1].name);
        for s in &out {
            assert!(VOCAB.contains(&s.name.as_str()));
        }
    }
}
