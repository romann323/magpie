//! Project abstraction and app-level settings persistence.
//!
//! A **project** is a single `.magpie` SQLite file the user chose the
//! location and name of. Magpie holds at most one project open at a
//! time and remembers the most-recently-used projects so it can
//! auto-open on the next launch.
//!
//! The user's project lives wherever they picked; the app's own
//! settings (last-project path, recent list, theme, font-size,
//! language) live in `%APPDATA%\com.magpie.app\app-settings.json`.

use crate::db::Db;
use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Recommended file extension for a Magpie project file.
pub const PROJECT_EXT: &str = "magpie";

/// Filename of the auto-migrated project created from a legacy central
/// `magpie.db`. Lives in `%APPDATA%\com.magpie.app\`.
pub const DEFAULT_PROJECT_FILENAME: &str = "Default.magpie";

/// Filename of the persisted app-level settings blob.
pub const SETTINGS_FILE_NAME: &str = "app-settings.json";

/// Maximum number of recent-project paths we remember. Older entries
/// fall off the end on `AppSettings::touch_recent`.
pub const RECENT_PROJECTS_MAX: usize = 10;

// ---------------------------------------------------------------------
//                          App-level settings
// ---------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    #[serde(default = "default_theme")]
    pub theme: Theme,
    #[serde(default = "default_font_size")]
    pub font_size: FontSize,
    #[serde(default = "default_language")]
    pub language: String,
    #[serde(default)]
    pub last_project_path: Option<PathBuf>,
    #[serde(default)]
    pub recent_projects: Vec<PathBuf>,
    /// When true, Magpie automatically runs AI-based tag assignment on
    /// every image in a library folder immediately after the folder's
    /// filesystem scan finishes. Off by default — opt in via
    /// **Settings → Auto-tag photos**.
    #[serde(default = "default_ai_auto_tag")]
    pub ai_auto_tag: bool,
}

fn default_theme() -> Theme {
    Theme::System
}
fn default_font_size() -> FontSize {
    FontSize::Medium
}
fn default_language() -> String {
    "en".into()
}
fn default_ai_auto_tag() -> bool {
    false
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            font_size: default_font_size(),
            language: default_language(),
            last_project_path: None,
            recent_projects: Vec::new(),
            ai_auto_tag: default_ai_auto_tag(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    System,
    Dark,
    Light,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum FontSize {
    Small,
    Medium,
    Large,
}

impl AppSettings {
    pub fn load(dir: &Path) -> Self {
        let path = dir.join(SETTINGS_FILE_NAME);
        match std::fs::read_to_string(&path) {
            Ok(s) => serde_json::from_str(&s).unwrap_or_else(|e| {
                tracing::warn!(error = %e, "failed to parse app-settings.json; falling back to defaults");
                Self::default()
            }),
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self, dir: &Path) -> AppResult<()> {
        std::fs::create_dir_all(dir)?;
        let path = dir.join(SETTINGS_FILE_NAME);
        let tmp = path.with_extension("json.tmp");
        let text = serde_json::to_string_pretty(self)
            .map_err(|e| AppError::Internal(format!("serialize settings: {e}")))?;
        std::fs::write(&tmp, text)?;
        std::fs::rename(&tmp, &path)?;
        Ok(())
    }

    /// Move `path` to the front of the recent list, deduplicating case-
    /// insensitively on Windows, and bound the list length.
    pub fn touch_recent(&mut self, path: &Path) {
        let canon = path.to_path_buf();
        self.recent_projects
            .retain(|p| !paths_equal_ignore_case(p, &canon));
        self.recent_projects.insert(0, canon);
        if self.recent_projects.len() > RECENT_PROJECTS_MAX {
            self.recent_projects.truncate(RECENT_PROJECTS_MAX);
        }
    }
}

fn paths_equal_ignore_case(a: &Path, b: &Path) -> bool {
    a.to_string_lossy().eq_ignore_ascii_case(&b.to_string_lossy())
}

/// Turn a project's absolute path into a stable, filesystem-safe key
/// used to name its thumbnail cache subdirectory. Case-insensitive on
/// Windows so `Default.magpie` and `default.magpie` don't split into
/// two caches.
///
/// The output is a short hex-encoded 64-bit hash — collisions are
/// astronomically unlikely and even if they occurred the fallout is
/// only a stale preview, not data loss.
pub fn thumb_cache_key(path: &Path) -> String {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    // Case-insensitive on Windows; case-sensitive elsewhere for parity
    // with the filesystem semantics we care about.
    #[cfg(target_os = "windows")]
    {
        path.to_string_lossy().to_ascii_lowercase().hash(&mut h);
    }
    #[cfg(not(target_os = "windows"))]
    {
        path.to_string_lossy().hash(&mut h);
    }
    format!("{:016x}", h.finish())
}

// ---------------------------------------------------------------------
//                           Project state
// ---------------------------------------------------------------------

/// Serializable descriptor of a project — sent to the frontend on
/// open/create/current queries.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectInfo {
    pub path: PathBuf,
    pub name: String,
}

impl ProjectInfo {
    pub fn from_path(path: &Path) -> Self {
        let name = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "Project".into());
        Self {
            path: path.to_path_buf(),
            name,
        }
    }
}

/// In-memory state: which project (if any) is currently open and a
/// handle to its DB. `None` when the user hasn't opened / created one
/// yet.
pub struct ProjectState {
    pub db: Option<Db>,
    pub info: Option<ProjectInfo>,
}

impl ProjectState {
    pub fn empty() -> Self {
        Self {
            db: None,
            info: None,
        }
    }
}

// ---------------------------------------------------------------------
//                    Open / create / save-as / close
// ---------------------------------------------------------------------

/// Create a brand-new project at `path`. Refuses to overwrite an
/// existing file.
pub fn create_project(path: &Path) -> AppResult<(Db, ProjectInfo)> {
    if path.exists() {
        return Err(AppError::BadInput(format!(
            "a file already exists at {}",
            path.display()
        )));
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let db = Db::open(path)?;
    Ok((db, ProjectInfo::from_path(path)))
}

/// Open an existing project file. Fails if the path doesn't exist or
/// isn't a valid Magpie DB.
pub fn open_project(path: &Path) -> AppResult<(Db, ProjectInfo)> {
    if !path.exists() {
        return Err(AppError::PathNotFound(path.display().to_string()));
    }
    let db = Db::open(path)?;
    Ok((db, ProjectInfo::from_path(path)))
}

/// Copy the currently-open DB to `new_path` using SQLite's online
/// backup API, then reopen from the new location so future writes hit
/// the new file. Caller must ensure the passed-in `Db` is the currently-
/// open project.
pub fn save_project_as(current: &Db, new_path: &Path) -> AppResult<(Db, ProjectInfo)> {
    if new_path.exists() {
        return Err(AppError::BadInput(format!(
            "a file already exists at {}",
            new_path.display()
        )));
    }
    if let Some(parent) = new_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    // Backup via SQLite's online backup API so WAL etc. are flushed
    // correctly and we don't have to close the source.
    current.with_conn(|src| {
        let mut dst = rusqlite::Connection::open(new_path)?;
        {
            let backup = rusqlite::backup::Backup::new(src, &mut dst)?;
            backup
                .run_to_completion(64, std::time::Duration::from_millis(0), None)
                .map_err(|e| AppError::Internal(format!("backup: {e}")))?;
        }
        Ok(())
    })?;
    let db = Db::open(new_path)?;
    Ok((db, ProjectInfo::from_path(new_path)))
}

// ---------------------------------------------------------------------
//                          Startup helper
// ---------------------------------------------------------------------

/// Resolve which project (if any) to auto-open when the app starts.
///
/// Behaviour:
/// - If the OS launched us with a `.magpie` file path as the first
///   command-line argument (e.g. the user double-clicked one in
///   Explorer), open that file — this takes precedence over
///   everything else.
/// - Otherwise, if `settings.last_project_path` is set (i.e. the user
///   has actively opened a project in a previous session) and the
///   file still exists, open it.
/// - If a legacy central `%APPDATA%\com.magpie.app\magpie.db` is
///   found, rename it to `Default.magpie` and add it to the recent
///   list but **don't auto-open** — the user still sees the welcome
///   screen on their first launch after upgrading and can click the
///   migrated project to open it.
/// - Otherwise, return `Ok(None)` so the frontend shows the welcome
///   screen.
///
/// On a soft failure (e.g. `last_project_path` no longer exists) the
/// setting is cleared and the caller should save `settings` back to
/// disk.
pub fn auto_open_on_startup(
    app_data_dir: &Path,
    settings: &mut AppSettings,
) -> AppResult<Option<(Db, ProjectInfo)>> {
    // 0) OS "open with" — the shell passes the file path as argv[1].
    if let Some(p) = launch_project_path_from_args() {
        match Db::open(&p) {
            Ok(db) => {
                tracing::info!(path = %p.display(), "opened project passed on the command line");
                settings.last_project_path = Some(p.clone());
                settings.touch_recent(&p);
                return Ok(Some((db, ProjectInfo::from_path(&p))));
            }
            Err(e) => {
                tracing::warn!(error = %e, path = %p.display(), "could not open project passed on the command line");
            }
        }
    }

    // 1) One-shot legacy migration — never auto-opens, just makes the
    //    migrated file discoverable via the recent-projects list.
    let legacy = app_data_dir.join(crate::db::DB_FILE_NAME);
    if legacy.exists() {
        let default_path = app_data_dir.join(DEFAULT_PROJECT_FILENAME);
        if !default_path.exists() {
            if let Err(e) = std::fs::rename(&legacy, &default_path) {
                tracing::warn!(
                    error = %e,
                    from = %legacy.display(),
                    to = %default_path.display(),
                    "could not migrate legacy magpie.db into Default.magpie; leaving in place"
                );
            } else {
                tracing::info!(
                    from = %legacy.display(),
                    to = %default_path.display(),
                    "migrated legacy magpie.db into Default.magpie; user must open it explicitly"
                );
            }
        }
        if default_path.exists() {
            settings.touch_recent(&default_path);
        }
    }

    // 2) Only auto-open when the user explicitly opened a project in a
    //    prior session.
    if let Some(p) = settings.last_project_path.clone() {
        if p.exists() {
            match Db::open(&p) {
                Ok(db) => {
                    settings.touch_recent(&p);
                    return Ok(Some((db, ProjectInfo::from_path(&p))));
                }
                Err(e) => {
                    tracing::warn!(error = %e, path = %p.display(), "last project failed to open");
                    settings.last_project_path = None;
                }
            }
        } else {
            tracing::info!(path = %p.display(), "last project no longer exists");
            settings.last_project_path = None;
        }
    }

    Ok(None)
}

/// Pick up the first argv entry that looks like a `.magpie` file that
/// actually exists on disk. On Windows the shell passes the path as
/// argv[1] when the user double-clicks a registered file type.
fn launch_project_path_from_args() -> Option<PathBuf> {
    for arg in std::env::args().skip(1) {
        let p = PathBuf::from(&arg);
        if p.exists()
            && p.extension()
                .and_then(|s| s.to_str())
                .map(|s| s.eq_ignore_ascii_case(PROJECT_EXT))
                .unwrap_or(false)
        {
            return Some(p);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thumb_cache_key_is_stable_and_project_specific() {
        let a = thumb_cache_key(Path::new(r"C:\Projects\Foo.magpie"));
        let a2 = thumb_cache_key(Path::new(r"C:\Projects\Foo.magpie"));
        let b = thumb_cache_key(Path::new(r"C:\Projects\Bar.magpie"));
        assert_eq!(a, a2, "same path must yield same key");
        assert_ne!(a, b, "different paths must yield different keys");
        // Non-empty, filesystem-safe (hex).
        assert!(!a.is_empty());
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn thumb_cache_key_is_case_insensitive_on_windows() {
        let a = thumb_cache_key(Path::new(r"C:\Projects\Foo.magpie"));
        let b = thumb_cache_key(Path::new(r"c:\PROJECTS\foo.MAGPIE"));
        assert_eq!(a, b);
    }

    #[test]
    fn touch_recent_dedupes_and_bounds() {
        let mut s = AppSettings::default();
        s.touch_recent(Path::new("a"));
        s.touch_recent(Path::new("b"));
        s.touch_recent(Path::new("A"));
        assert_eq!(
            s.recent_projects,
            vec![PathBuf::from("A"), PathBuf::from("b")]
        );
    }

    #[test]
    fn save_and_load_roundtrip() {
        let dir = std::env::temp_dir()
            .join("magpie_settings_test")
            .join(format!("t{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let mut s = AppSettings::default();
        s.theme = Theme::Dark;
        s.font_size = FontSize::Large;
        s.touch_recent(Path::new("x"));
        s.save(&dir).unwrap();
        let loaded = AppSettings::load(&dir);
        assert_eq!(loaded, s);
    }
}
