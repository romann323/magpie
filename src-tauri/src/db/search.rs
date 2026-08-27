//! Cross-folder search + tag aggregation.
//!
//! Everything in this module runs against the *registry* connection
//! with every library `ATTACH DATABASE`-ed as `f<id>`. It builds
//! `UNION ALL`s across attached schemas at runtime, filtered by the
//! same [`ImageFilter`] the frontend has always used.
//!
//! The trick that keeps this cheap: each branch of the union embeds
//! its folder ID into a synthetic `gid` column:
//!
//! ```text
//! SELECT (folder_id * 1_000_000_000 + id) AS gid, ... FROM f1.images
//! UNION ALL
//! SELECT (folder_id * 1_000_000_000 + id) AS gid, ... FROM f2.images
//! ```
//!
//! Downstream sort/pagination sees a single flat result set. The
//! per-folder root path is stitched onto each row's relative path in
//! Rust after the fetch.

use crate::db::library::MetadataPatch;
use crate::db::pool::LibraryPool;
use crate::db::{registry, FOLDER_ID_MULT};
use crate::error::AppResult;
use crate::types::*;
use rusqlite::{params_from_iter, types::Value};
use std::collections::HashMap;
use std::path::PathBuf;

pub fn list_all_tags(pool: &LibraryPool, prefix: Option<&str>) -> AppResult<Vec<TagStats>> {
    pool.with_registry(|conn| {
        let folders = registry::list_folders(conn)?
            .into_iter()
            .filter(|f| f.is_available)
            .collect::<Vec<_>>();
        if folders.is_empty() {
            return Ok(Vec::new());
        }
        let prefix_pat = prefix
            .map(|p| p.trim().to_string())
            .filter(|p| !p.is_empty());

        // UNION ALL of tag-name + counts across every attached folder,
        // then a top-level GROUP BY that merges "Beach" and "beach"
        // from different libraries into a single row.
        let mut parts: Vec<String> = Vec::with_capacity(folders.len());
        for f in &folders {
            let alias = format!("f{}", f.id);
            let where_ = if prefix_pat.is_some() {
                format!("WHERE t.name LIKE ? COLLATE NOCASE")
            } else {
                String::new()
            };
            parts.push(format!(
                "SELECT t.name AS name, COUNT(it.image_id) AS c
                 FROM {alias}.tags t
                 LEFT JOIN {alias}.image_tags it ON it.tag_id = t.id
                 {where_}
                 GROUP BY t.id"
            ));
        }
        let inner = parts.join(" UNION ALL ");
        let sql = format!(
            "SELECT name, SUM(c) AS total
             FROM ({inner})
             GROUP BY name COLLATE NOCASE
             ORDER BY total DESC, name COLLATE NOCASE
             LIMIT 500"
        );

        let args: Vec<Value> = if let Some(p) = &prefix_pat {
            (0..folders.len())
                .map(|_| Value::Text(format!("{}%", p)))
                .collect()
        } else {
            Vec::new()
        };

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params_from_iter(args.iter()), |row| {
            Ok(TagStats {
                name: row.get(0)?,
                count: row.get(1)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    })
}

pub fn query_images(
    pool: &LibraryPool,
    filter: &ImageFilter,
    sort: &ImageSort,
    page: &Pagination,
) -> AppResult<Page<ImageSummary>> {
    pool.with_registry(|conn| {
        let folders = registry::list_folders(conn)?
            .into_iter()
            .filter(|f| f.is_available)
            .collect::<Vec<_>>();
        if folders.is_empty() {
            return Ok(Page {
                items: Vec::new(),
                total: 0,
                offset: page.offset,
                limit: page.limit,
            });
        }
        let folder_paths: HashMap<i64, String> =
            folders.iter().map(|f| (f.id, f.path.clone())).collect();

        // Optional folder-scope filter narrows the UNION.
        let scope: Vec<&registry::LibraryFolderRow> = match &filter.folder_ids {
            Some(ids) if !ids.is_empty() => folders
                .iter()
                .filter(|f| ids.contains(&f.id))
                .collect(),
            _ => folders.iter().collect(),
        };
        if scope.is_empty() {
            return Ok(Page {
                items: Vec::new(),
                total: 0,
                offset: page.offset,
                limit: page.limit,
            });
        }

        let mut union_parts: Vec<String> = Vec::with_capacity(scope.len());
        let mut all_args: Vec<Value> = Vec::new();
        for f in &scope {
            let alias = format!("f{}", f.id);
            let (branch_sql, branch_args) = build_branch(&alias, f.id, filter);
            union_parts.push(branch_sql);
            all_args.extend(branch_args);
        }
        let union_sql = union_parts.join(" UNION ALL ");
        let order_by = match (sort.by, sort.dir) {
            (SortBy::TakenAt, SortDir::Asc) => "COALESCE(taken_at, mtime_ms) ASC, gid ASC",
            (SortBy::TakenAt, SortDir::Desc) => "COALESCE(taken_at, mtime_ms) DESC, gid DESC",
            (SortBy::Filename, SortDir::Asc) => "filename COLLATE NOCASE ASC, gid ASC",
            (SortBy::Filename, SortDir::Desc) => "filename COLLATE NOCASE DESC, gid DESC",
            (SortBy::AddedAt, SortDir::Asc) => "gid ASC",
            (SortBy::AddedAt, SortDir::Desc) => "gid DESC",
            (SortBy::Size, SortDir::Asc) => "size_bytes ASC, gid ASC",
            (SortBy::Size, SortDir::Desc) => "size_bytes DESC, gid DESC",
        };
        let count_sql = format!("SELECT COUNT(*) FROM ({union_sql})");
        let sql = format!(
            "SELECT * FROM ({union_sql}) ORDER BY {order_by} LIMIT ? OFFSET ?"
        );

        let total: i64 =
            conn.query_row(&count_sql, params_from_iter(all_args.iter()), |r| r.get(0))?;

        let mut paged_args = all_args.clone();
        paged_args.push(Value::Integer(page.limit));
        paged_args.push(Value::Integer(page.offset));

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params_from_iter(paged_args.iter()), |row| {
            let gid: i64 = row.get(0)?;
            let folder_id: i64 = row.get(1)?;
            let rel_path: String = row.get(2)?;
            let filename: String = row.get(3)?;
            let ext: String = row.get(4)?;
            let size_bytes: i64 = row.get(5)?;
            let mtime_ms: i64 = row.get(6)?;
            let width: Option<i64> = row.get(7)?;
            let height: Option<i64> = row.get(8)?;
            let content_hash: Option<String> = row.get(9)?;
            let taken_at: Option<i64> = row.get(10)?;
            let title: Option<String> = row.get(11)?;

            let root = folder_paths.get(&folder_id).cloned().unwrap_or_default();
            let abs = if root.is_empty() {
                rel_path
            } else {
                PathBuf::from(&root)
                    .join(&rel_path)
                    .to_string_lossy()
                    .into_owned()
            };
            Ok(ImageSummary {
                id: gid,
                folder_id,
                path: abs,
                filename,
                ext,
                width,
                height,
                size_bytes,
                mtime_ms,
                taken_at,
                title,
                content_hash,
            })
        })?;
        let items = rows.collect::<Result<Vec<_>, _>>()?;
        Ok(Page {
            items,
            total,
            offset: page.offset,
            limit: page.limit,
        })
    })
}

/// Build a single per-folder branch of the cross-folder UNION.
///
/// Every branch has the same column shape:
///   `gid, folder_id, rel_path, filename, ext, size_bytes, mtime_ms,
///    width, height, content_hash, taken_at, title`
///
/// All filter clauses that touch tags or FTS reference the per-branch
/// tables via `{alias}.tags`, `{alias}.image_tags`, `{alias}.images_fts`
/// so each library only searches its own tag namespace.
fn build_branch(alias: &str, folder_id: i64, filter: &ImageFilter) -> (String, Vec<Value>) {
    let mut where_clauses: Vec<String> = vec!["missing = 0".into()];
    let mut args: Vec<Value> = Vec::new();

    if let Some(after) = filter.taken_after {
        where_clauses.push("taken_at >= ?".into());
        args.push(Value::Integer(after));
    }
    if let Some(before) = filter.taken_before {
        where_clauses.push("taken_at <= ?".into());
        args.push(Value::Integer(before));
    }
    if let Some(ext) = &filter.ext {
        if !ext.is_empty() {
            let placeholders = (0..ext.len())
                .map(|_| "?".to_string())
                .collect::<Vec<_>>()
                .join(",");
            where_clauses.push(format!("ext IN ({})", placeholders));
            for e in ext {
                args.push(Value::Text(e.to_lowercase()));
            }
        }
    }
    if let Some(true) = filter.has_title {
        where_clauses.push("title IS NOT NULL AND title <> ''".into());
    }
    if let Some(fts) = &filter.fts {
        let q = fts.trim();
        if !q.is_empty() {
            where_clauses.push(format!(
                "id IN (SELECT rowid FROM {alias}.images_fts WHERE {alias}.images_fts MATCH ?)"
            ));
            args.push(Value::Text(fts_query_from_user(q)));
        }
    }
    if let Some(all) = &filter.tags_all {
        for t in all {
            let name = t.trim();
            if name.is_empty() {
                continue;
            }
            where_clauses.push(format!(
                "id IN (SELECT it.image_id FROM {alias}.image_tags it
                        JOIN {alias}.tags t ON t.id = it.tag_id
                        WHERE t.name = ? COLLATE NOCASE)"
            ));
            args.push(Value::Text(name.to_string()));
        }
    }
    if let Some(any) = &filter.tags_any {
        let names: Vec<&str> = any
            .iter()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();
        if !names.is_empty() {
            let placeholders = (0..names.len())
                .map(|_| "?".to_string())
                .collect::<Vec<_>>()
                .join(",");
            where_clauses.push(format!(
                "id IN (SELECT it.image_id FROM {alias}.image_tags it
                        JOIN {alias}.tags t ON t.id = it.tag_id
                        WHERE t.name IN ({}) COLLATE NOCASE)",
                placeholders
            ));
            for n in names {
                args.push(Value::Text(n.to_string()));
            }
        }
    }
    if let Some(none) = &filter.tags_none {
        for t in none {
            let name = t.trim();
            if name.is_empty() {
                continue;
            }
            where_clauses.push(format!(
                "id NOT IN (SELECT it.image_id FROM {alias}.image_tags it
                            JOIN {alias}.tags t ON t.id = it.tag_id
                            WHERE t.name = ? COLLATE NOCASE)"
            ));
            args.push(Value::Text(name.to_string()));
        }
    }

    let where_sql = where_clauses.join(" AND ");
    let sql = format!(
        "SELECT ({fid} * {mult} + id) AS gid,
                {fid} AS folder_id,
                rel_path,
                filename,
                ext,
                size_bytes,
                mtime_ms,
                width,
                height,
                content_hash,
                taken_at,
                title
         FROM {alias}.images
         WHERE {where_sql}",
        fid = folder_id,
        mult = FOLDER_ID_MULT,
    );
    (sql, args)
}

/// Escape user text for FTS5. Wrap tokens in double quotes so special
/// characters are literal, and append `*` to the last token for prefix
/// search — mirrors the pre-redesign behaviour.
fn fts_query_from_user(s: &str) -> String {
    let tokens: Vec<String> = s
        .split_whitespace()
        .map(|t| t.replace('"', ""))
        .filter(|t| !t.is_empty())
        .collect();
    if tokens.is_empty() {
        return String::new();
    }
    let mut parts: Vec<String> = tokens.iter().map(|t| format!("\"{}\"", t)).collect();
    if let Some(last) = parts.last_mut() {
        if last.ends_with('"') {
            last.pop();
        }
        last.push_str("\"*");
    }
    parts.join(" ")
}

/// Look up an image by its packed global ID. Returns
/// `(folder_id, local_id, folder_root, image_row)`.
pub fn get_image_by_gid(
    pool: &LibraryPool,
    gid: i64,
) -> AppResult<Option<(i64, i64, PathBuf, crate::db::library::ImageRow)>> {
    let (folder_id, local_id) = crate::db::unpack_global_id(gid);
    let folder = pool.folder(folder_id)?;
    let root = PathBuf::from(&folder.path);
    let lib = pool.library(folder_id)?;
    let conn = lib.lock()?;
    let row = crate::db::library::get_image_row(&conn, local_id)?;
    Ok(row.map(|r| (folder_id, local_id, root, r)))
}

pub fn apply_metadata_patch_by_gid(
    pool: &LibraryPool,
    gid: i64,
    patch: &MetadataPatch,
) -> AppResult<()> {
    let (folder_id, local_id) = crate::db::unpack_global_id(gid);
    let lib = pool.library(folder_id)?;
    let mut conn = lib.lock()?;
    crate::db::library::apply_metadata_patch(&mut conn, local_id, patch)
}

/// Group `[gid, ...]` by folder for batch delete / update.
pub fn group_gids_by_folder(gids: &[i64]) -> HashMap<i64, Vec<i64>> {
    let mut out: HashMap<i64, Vec<i64>> = HashMap::new();
    for g in gids {
        let (f, l) = crate::db::unpack_global_id(*g);
        out.entry(f).or_default().push(l);
    }
    out
}
