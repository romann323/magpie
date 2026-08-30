pub mod auto_tag;
pub mod formats;
pub mod metadata;
pub mod project;
pub mod scanner;
pub mod thumbnail;

use crate::db::Db;
use crate::error::{AppError, AppResult};
use formats::FormatRegistry;
use project::{AppSettings, ProjectInfo, ProjectState};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Global services shared with every Tauri command handler.
///
/// A project (`.magpie` file) may or may not be open at any given
/// moment; `project` holds the currently-open one. `settings` is the
/// app-level configuration blob persisted under `%APPDATA%`.
///
/// Thumbnails are cached under `<thumbs_root>/<per-project-key>/…` so
/// two projects that happen to reuse the same `image_id` never see
/// each other's cached previews.
pub struct AppServices {
    project: Mutex<ProjectState>,
    settings: Mutex<AppSettings>,
    magnifier: Mutex<crate::types::MagnifierContext>,
    pub app_data_dir: PathBuf,
    thumbs_root: PathBuf,
    pub formats: Arc<FormatRegistry>,
    /// Serializes automatic-AI-tagging passes so that when the user
    /// adds several library folders in quick succession the AI worker
    /// processes them one-at-a-time in FIFO order rather than
    /// starting a parallel pass per folder. Filesystem scans stay
    /// parallel — this gate only touches AI. See
    /// [`crate::core::auto_tag`].
    pub auto_tag_gate: Arc<tokio::sync::Mutex<()>>,
}

impl AppServices {
    pub fn new(app_data_dir: PathBuf) -> anyhow::Result<Arc<Self>> {
        let thumbs_root = app_data_dir.join("thumbs");
        std::fs::create_dir_all(&thumbs_root)?;

        let mut settings = AppSettings::load(&app_data_dir);
        let mut state = ProjectState::empty();
        match project::auto_open_on_startup(&app_data_dir, &mut settings) {
            Ok(Some((db, info))) => {
                tracing::info!(project = %info.path.display(), "auto-opened project on startup");
                state.db = Some(db);
                state.info = Some(info);
            }
            Ok(None) => {
                tracing::info!("no project auto-opened; waiting for user to create/open one");
            }
            Err(e) => {
                tracing::warn!(error = %e, "auto-open failed; starting without a project");
            }
        }
        if let Err(e) = settings.save(&app_data_dir) {
            tracing::warn!(error = %e, "could not persist app-settings.json on startup");
        }

        let formats = Arc::new(FormatRegistry::new());
        Ok(Arc::new(Self {
            project: Mutex::new(state),
            settings: Mutex::new(settings),
            magnifier: Mutex::new(crate::types::MagnifierContext::default()),
            app_data_dir,
            thumbs_root,
            formats,
            auto_tag_gate: Arc::new(tokio::sync::Mutex::new(())),
        }))
    }

    // -----------------------------------------------------------------
    //                     Magnifier window context
    // -----------------------------------------------------------------

    /// Read the current magnifier window's context (which image the
    /// magnifier should display + the filter/sort of the list it is
    /// navigating). Returns a default (empty) context on first read.
    pub fn magnifier_context(&self) -> AppResult<crate::types::MagnifierContext> {
        let g = self
            .magnifier
            .lock()
            .map_err(|_| AppError::Pool("magnifier state mutex poisoned".into()))?;
        Ok(g.clone())
    }

    /// Overwrite the magnifier context. Called by the main window
    /// before spawning the magnifier window so the popup knows what to
    /// display and which set of images to walk through.
    pub fn set_magnifier_context(
        &self,
        ctx: crate::types::MagnifierContext,
    ) -> AppResult<()> {
        let mut g = self
            .magnifier
            .lock()
            .map_err(|_| AppError::Pool("magnifier state mutex poisoned".into()))?;
        *g = ctx;
        Ok(())
    }

    /// Update just the "current image" pointer while keeping the same
    /// list context. Called by the magnifier window when the user
    /// navigates with the arrow keys.
    pub fn set_magnifier_current(&self, image_id: Option<i64>) -> AppResult<()> {
        let mut g = self
            .magnifier
            .lock()
            .map_err(|_| AppError::Pool("magnifier state mutex poisoned".into()))?;
        g.image_id = image_id;
        Ok(())
    }

    /// Directory that holds the cached thumbnails for the currently-
    /// open project. Fails with `NoProjectOpen` if no project is open.
    ///
    /// The directory is derived from a stable hash of the project's
    /// absolute path so:
    /// - Two projects with different paths never share cache entries.
    /// - Reopening the same project reuses its thumbnails.
    pub fn thumb_cache_dir(&self) -> AppResult<PathBuf> {
        let info = self
            .current_project()?
            .ok_or(AppError::NoProjectOpen)?;
        let key = project::thumb_cache_key(&info.path);
        let dir = self.thumbs_root.join(key);
        std::fs::create_dir_all(&dir)?;
        Ok(dir)
    }

    /// Root of the shared thumbnails folder. Individual projects live
    /// in subdirectories under this — see [`thumb_cache_dir`].
    pub fn thumbs_root(&self) -> &Path {
        &self.thumbs_root
    }

    // -----------------------------------------------------------------
    //                       Project accessors
    // -----------------------------------------------------------------

    /// Returns a cheap clone of the current project's `Db` handle.
    /// `NoProjectOpen` when the user hasn't opened / created one.
    pub fn db(&self) -> AppResult<Db> {
        let g = self
            .project
            .lock()
            .map_err(|_| AppError::Pool("project state mutex poisoned".into()))?;
        g.db.clone().ok_or(AppError::NoProjectOpen)
    }

    /// Returns a copy of the current project's descriptor, if any.
    pub fn current_project(&self) -> AppResult<Option<ProjectInfo>> {
        let g = self
            .project
            .lock()
            .map_err(|_| AppError::Pool("project state mutex poisoned".into()))?;
        Ok(g.info.clone())
    }

    /// Swap the currently-open project. Passing `None` closes it.
    /// The app-settings blob is updated (last_project_path + recent
    /// list) and persisted.
    pub fn set_project(&self, new: Option<(Db, ProjectInfo)>) -> AppResult<Option<ProjectInfo>> {
        let (db, info) = match new {
            Some((d, i)) => (Some(d), Some(i)),
            None => (None, None),
        };
        {
            let mut g = self
                .project
                .lock()
                .map_err(|_| AppError::Pool("project state mutex poisoned".into()))?;
            g.db = db;
            g.info = info.clone();
        }
        {
            let mut s = self
                .settings
                .lock()
                .map_err(|_| AppError::Pool("settings mutex poisoned".into()))?;
            match &info {
                Some(i) => {
                    s.last_project_path = Some(i.path.clone());
                    s.touch_recent(&i.path);
                }
                None => {
                    s.last_project_path = None;
                }
            }
            if let Err(e) = s.save(&self.app_data_dir) {
                tracing::warn!(error = %e, "could not persist app-settings.json");
            }
        }
        Ok(info)
    }

    // -----------------------------------------------------------------
    //                       Settings accessors
    // -----------------------------------------------------------------

    pub fn get_settings(&self) -> AppResult<AppSettings> {
        let g = self
            .settings
            .lock()
            .map_err(|_| AppError::Pool("settings mutex poisoned".into()))?;
        Ok(g.clone())
    }

    /// Merge a partial patch into the current settings, save, and
    /// return the new value.
    pub fn update_settings(
        &self,
        f: impl FnOnce(&mut AppSettings),
    ) -> AppResult<AppSettings> {
        let mut g = self
            .settings
            .lock()
            .map_err(|_| AppError::Pool("settings mutex poisoned".into()))?;
        f(&mut g);
        g.save(&self.app_data_dir)?;
        Ok(g.clone())
    }
}

/// Convenience: which extensions the `image` crate can decode. Used
/// only by the thumbnail pipeline to decide whether it should try to
/// render a preview. All *scannable* extensions come from
/// [`FormatRegistry::all_extensions`] instead.
pub fn is_processable_by_image_crate(ext: &str) -> bool {
    matches!(
        ext.to_ascii_lowercase().as_str(),
        "jpg" | "jpeg" | "jpe" | "jfif" | "jif"
        | "png" | "webp" | "gif" | "bmp"
        | "tif" | "tiff"
    )
}
