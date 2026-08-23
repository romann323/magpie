//! Two-tier SQLite storage.
//!
//! - [`registry::RegistryDb`] — a small per-machine DB
//!   (`%APPDATA%\com.magpie.app\registry.db`) that tracks which library
//!   folders the user has added, smart collections, and misc app
//!   settings. This is the "control plane".
//! - [`library::LibraryDb`] — one SQLite file per library folder,
//!   stored at `<folder>/.magpie/library.db`. Owns the folder's images,
//!   tags, and FTS index. Fully self-contained; moving the folder
//!   moves the DB with it.
//! - [`pool::LibraryPool`] — glue layer. Opens the registry once,
//!   `ATTACH DATABASE`es every registered library so cross-folder
//!   search runs off a single connection, and hands out per-folder
//!   writer connections on demand.
//! - [`legacy_migration::migrate_legacy_central_db`] — one-shot
//!   migration for users upgrading from the pre-redesign build that
//!   stored everything in one central `library.db`.
//!
//! See `docs/developer/src/design/db-redesign.md` for the full design
//! rationale.

pub mod legacy_migration;
pub mod library;
pub mod pool;
pub mod registry;
pub mod search;

/// Packing scheme for cross-folder globally-unique image IDs surfaced
/// to the frontend. Local per-library IDs (autoincrement) are combined
/// with the folder ID from the registry via
/// `folder_id * FOLDER_ID_MULT + local_id`. All packed IDs fit inside
/// JavaScript's safe integer range (2⁵³) up to ~9 M folders and ~1 B
/// images per folder.
pub const FOLDER_ID_MULT: i64 = 1_000_000_000;

#[inline]
pub fn pack_global_id(folder_id: i64, local_id: i64) -> i64 {
    folder_id * FOLDER_ID_MULT + local_id
}

#[inline]
pub fn unpack_global_id(global: i64) -> (i64, i64) {
    (global / FOLDER_ID_MULT, global % FOLDER_ID_MULT)
}

#[cfg(test)]
mod id_pack_tests {
    use super::*;

    #[test]
    fn roundtrip() {
        for (f, l) in [(1, 1), (1, 42), (7, 123456), (999_999, 987_654_321)] {
            let g = pack_global_id(f, l);
            let (f2, l2) = unpack_global_id(g);
            assert_eq!((f2, l2), (f, l), "roundtrip failed for {f},{l}");
        }
    }

    #[test]
    fn stays_within_js_safe_range() {
        // 2^53 = 9_007_199_254_740_992
        let max_f = 9_000_000_i64;
        let max_l = 999_999_999_i64;
        let g = pack_global_id(max_f, max_l);
        assert!(g < 9_007_199_254_740_992, "packed ID exceeds Number.MAX_SAFE_INTEGER: {g}");
    }
}
