pub mod commands;
pub mod core;
pub mod db;
pub mod error;
pub mod types;

use crate::core::AppServices;
use std::sync::{Arc, Mutex};
use tauri::Manager;

/// Writer that appends every tracing event to a shared file handle.
/// Implements `MakeWriter` via a wrapping newtype.
struct SharedFileWriter(Arc<Mutex<std::fs::File>>);

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for SharedFileWriter {
    type Writer = FileLine;
    fn make_writer(&'a self) -> Self::Writer {
        FileLine(self.0.clone())
    }
}

struct FileLine(Arc<Mutex<std::fs::File>>);
impl std::io::Write for FileLine {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut f = self
            .0
            .lock()
            .map_err(|_| std::io::Error::other("log mutex poisoned"))?;
        f.write(buf)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        let mut f = self
            .0
            .lock()
            .map_err(|_| std::io::Error::other("log mutex poisoned"))?;
        f.flush()
    }
}

fn init_logging() {
    // Log to %APPDATA%\com.picorg.picorg\logs\picorg.log so we can diagnose
    // release builds (Windows GUI apps have no console).
    let log_dir = dirs::data_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("com.picorg.picorg")
        .join("logs");
    let _ = std::fs::create_dir_all(&log_dir);
    let log_path = log_dir.join("picorg.log");

    let file_writer_opt = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .ok()
        .map(|f| SharedFileWriter(Arc::new(Mutex::new(f))));

    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,picorg_lib=debug"));

    match file_writer_opt {
        Some(fw) => {
            tracing_subscriber::fmt()
                .with_env_filter(filter)
                .with_ansi(false)
                .with_writer(fw)
                .init();
        }
        None => {
            tracing_subscriber::fmt().with_env_filter(filter).init();
        }
    }

    tracing::info!(?log_path, "logging initialized");
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    init_logging();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let app_data_dir = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| std::env::temp_dir().join("PicOrg"));

            let services = AppServices::new(app_data_dir)
                .expect("failed to initialize PicOrg services");

            tracing::info!(
                app_data_dir = ?services.app_data_dir,
                "PicOrg started"
            );

            app.manage(services);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::library::add_library_folder,
            commands::library::remove_library_folder,
            commands::library::list_library_folders,
            commands::library::rescan_folder,
            commands::library::rescan_all,
            commands::images::query_images,
            commands::images::get_image,
            commands::images::update_image_metadata,
            commands::images::batch_update_metadata,
            commands::images::delete_images,
            commands::tags::list_tags,
            commands::tags::rename_tag,
            commands::tags::delete_tag,
            commands::collections::list_smart_collections,
            commands::collections::create_smart_collection,
            commands::collections::delete_smart_collection,
            commands::thumbs::get_thumb_path,
            commands::thumbs::get_image_path,
            commands::diag::log_frontend,
        ])
        .run(tauri::generate_context!())
        .expect("error while running PicOrg");
}
