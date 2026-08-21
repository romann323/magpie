//! Diagnostic tool: probes and exercises the Windows Shell property
//! fallback that Magpie uses to write tags into formats without a native
//! handler (RAW families, video, PDF, HEIC, …).
//!
//! Usage:
//!     cargo run -q --example dump_shell -- "C:\path\to\photo.x3f"
//!     cargo run -q --example dump_shell -- "C:\path\to\clip.mp4" write "vacation,beach" "Trip title"
//!
//! Positional arguments:
//!     1. path                (required)
//!     2. mode                "read" (default) or "write"
//!     3. tags (comma sep)    only used when mode = "write"
//!     4. title               only used when mode = "write"

use desktop_lib::core::formats::{win_shell, UserMeta};
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: dump_shell <path> [read|write] [tags,csv] [title]");
        std::process::exit(2);
    }
    let path = PathBuf::from(&args[0]);
    let mode = args.get(1).map(|s| s.as_str()).unwrap_or("read");

    println!("File   : {}", path.display());
    println!(
        "Exists : {}   size = {}",
        path.exists(),
        std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0)
    );

    println!("--- Shell probe: can_write_tags ---");
    let can = win_shell::can_write_tags(&path);
    println!("  can_write_tags = {can}");

    println!("--- Shell read: read_user_meta ---");
    match win_shell::read_user_meta(&path) {
        Some(um) => {
            println!("  title = {:?}", um.title);
            println!("  tags  = {:?}", um.tags);
        }
        None => println!("  (no title/tags reported by the property store)"),
    }

    if mode == "write" {
        let tags_csv = args.get(2).cloned().unwrap_or_default();
        let title = args.get(3).cloned();
        let tags: Vec<String> = tags_csv
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        let meta = UserMeta { title, tags };
        println!("--- Shell write: write_user_meta({:?}) ---", meta);
        match win_shell::write_user_meta(&path, &meta) {
            Ok(()) => {
                println!("  Ok! Re-reading to verify…");
                match win_shell::read_user_meta(&path) {
                    Some(um) => {
                        println!("  title = {:?}", um.title);
                        println!("  tags  = {:?}", um.tags);
                    }
                    None => println!("  (post-write read returned no metadata)"),
                }
            }
            Err(e) => println!("  err: {e}"),
        }
    }
}
