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

    // Existing DB — check version. Future migrations plug in here.
    let ver: i64 = conn
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
    // v == SCHEMA_VERSION → nothing to do.
    Ok(())
}
