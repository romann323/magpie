//! Schema for the central Magpie database.
//!
//! [`apply`] is called by [`super::Db::open`] on every launch. On a
//! fresh file it runs the DDL below. On an existing file it verifies
//! that the schema version matches [`super::SCHEMA_VERSION`]; add
//! future migrations there when the schema evolves.

use crate::error::{AppError, AppResult};
use rusqlite::{params, Connection};

const SCHEMA_SQL: &str = include_str!("schema.sql");

pub fn apply(conn: &Connection) -> AppResult<()> {
    // Detect a fresh DB by the absence of the schema_meta table.
    let is_fresh: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master
             WHERE type = 'table' AND name = 'schema_meta'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map(|n| n == 0)
        .unwrap_or(true);

    if is_fresh {
        conn.execute_batch(SCHEMA_SQL)?;
        let now = chrono::Utc::now().timestamp_millis();
        conn.execute(
            "INSERT INTO schema_meta (id, magpie_version, schema_version, created_at)
             VALUES (1, ?1, ?2, ?3)",
            params![env!("CARGO_PKG_VERSION"), super::SCHEMA_VERSION, now],
        )?;
        tracing::info!(
            version = super::SCHEMA_VERSION,
            "initialised fresh Magpie DB schema"
        );
        return Ok(());
    }

    // Existing DB — check version and run upgrades one hop at a time.
    let mut ver: i64 = conn
        .query_row(
            "SELECT schema_version FROM schema_meta WHERE id = 1",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);
    if ver > super::SCHEMA_VERSION {
        return Err(AppError::Internal(format!(
            "magpie.db was created by a newer Magpie (schema v{ver}); \
             current app supports v{}",
            super::SCHEMA_VERSION,
        )));
    }

    while ver < super::SCHEMA_VERSION {
        match ver {
            1 => {
                migrate_v1_to_v2(conn)?;
                ver = 2;
            }
            2 => {
                migrate_v2_to_v3(conn)?;
                ver = 3;
            }
            other => {
                return Err(AppError::Internal(format!(
                    "no migration path from schema v{other} to v{}",
                    super::SCHEMA_VERSION,
                )));
            }
        }
        conn.execute(
            "UPDATE schema_meta SET schema_version = ?1, magpie_version = ?2 WHERE id = 1",
            params![ver, env!("CARGO_PKG_VERSION")],
        )?;
        tracing::info!(from = ver - 1, to = ver, "applied Magpie DB migration");
    }
    Ok(())
}

/// v1 → v2: split `image_tags` into (image, tag, source) rows.
///
/// Existing rows have no provenance, so they are all marked as
/// `'user'`. Rationale: those rows are what the user has been editing
/// (and what the pre-split scanner would have wiped on next rescan);
/// keeping them as `'user'` guarantees no typed edit is lost. On the
/// next scan the format handlers add any missing `'auto'` rows
/// alongside without touching user rows.
/// v2 → v3: add `ai_tagged_at` (INTEGER, ms since epoch) and
/// `ai_tag_hash` (TEXT) columns to `images`. Both nullable so
/// existing rows migrate in place without needing a default.
fn migrate_v2_to_v3(conn: &Connection) -> AppResult<()> {
    conn.execute_batch(
        "BEGIN;
         ALTER TABLE images ADD COLUMN ai_tagged_at INTEGER;
         ALTER TABLE images ADD COLUMN ai_tag_hash  TEXT;
         COMMIT;",
    )?;
    Ok(())
}

fn migrate_v1_to_v2(conn: &Connection) -> AppResult<()> {
    // SQLite can't alter a PK in place — rebuild the table.
    conn.execute_batch(
        "BEGIN;
         CREATE TABLE image_tags_new (
             image_id INTEGER NOT NULL REFERENCES images(id) ON DELETE CASCADE,
             tag_id   INTEGER NOT NULL REFERENCES tags(id)   ON DELETE CASCADE,
             source   TEXT    NOT NULL CHECK (source IN ('auto','user')),
             PRIMARY KEY (image_id, tag_id, source)
         );
         INSERT INTO image_tags_new (image_id, tag_id, source)
             SELECT image_id, tag_id, 'user' FROM image_tags;
         DROP TABLE image_tags;
         ALTER TABLE image_tags_new RENAME TO image_tags;
         CREATE INDEX IF NOT EXISTS idx_image_tags_tag    ON image_tags(tag_id);
         CREATE INDEX IF NOT EXISTS idx_image_tags_source ON image_tags(image_id, source);
         COMMIT;",
    )?;
    Ok(())
}
