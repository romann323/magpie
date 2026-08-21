//! Diagnostic: for every file passed on argv (or every file in the given
//! directory), report whether the Windows Shell property system can
//! actually write `System.Title` and `System.Keywords` on it.
//!
//! We copy each sample to a scratch dir, then try a full round-trip:
//! probe → write → read-back → compare. This is the "definitive" answer
//! per file, not just an educated guess based on GPS_READWRITE opening.
//!
//! Usage:
//!     cargo run -q --example probe_shell_matrix -- samples/format-samples/files

use desktop_lib::core::formats::{win_shell, UserMeta};
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: probe_shell_matrix <dir_or_file> [dir_or_file...]");
        std::process::exit(2);
    }

    let scratch = std::env::temp_dir().join("magpie-shell-probe");
    let _ = std::fs::remove_dir_all(&scratch);
    std::fs::create_dir_all(&scratch).unwrap();

    let mut files: Vec<PathBuf> = Vec::new();
    for a in args {
        let p = PathBuf::from(&a);
        if p.is_dir() {
            for entry in walkdir(&p) {
                if entry.is_file() {
                    files.push(entry);
                }
            }
        } else {
            files.push(p);
        }
    }

    println!(
        "{:<8} | {:<10} | {:<8} | {:<10} | {}",
        "EXT", "PROBE(RW)", "WRITE", "READBACK", "PATH"
    );
    println!("{}", "-".repeat(90));

    for src in files {
        let ext = src
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let name = src.file_name().unwrap().to_string_lossy().to_string();
        let dst = scratch.join(&name);
        if let Err(e) = std::fs::copy(&src, &dst) {
            println!("{:<8} | copy fail: {e}", ext);
            continue;
        }

        let probe = win_shell::can_write_tags(&dst);
        let title = "MagpieProbe";
        let tags = vec!["magpie".to_string(), "probe".to_string(), ext.clone()];
        let meta = UserMeta {
            title: Some(title.to_string()),
            tags: tags.clone(),
        };
        let write_ok = win_shell::write_user_meta(&dst, &meta).is_ok();
        let readback = if write_ok {
            match win_shell::read_user_meta(&dst) {
                Some(um) => {
                    let title_matches = um.title.as_deref() == Some(title);
                    let mut got = um.tags.clone();
                    got.sort();
                    let mut want = tags.clone();
                    want.sort();
                    let tags_match = got == want;
                    if title_matches && tags_match {
                        "both".to_string()
                    } else if title_matches {
                        "title-only".to_string()
                    } else if tags_match {
                        "tags-only".to_string()
                    } else {
                        format!("wrong({:?}, {:?})", um.title, um.tags)
                    }
                }
                None => "empty".to_string(),
            }
        } else {
            "-".to_string()
        };
        println!(
            "{:<8} | {:<10} | {:<8} | {:<10} | {}",
            ext,
            if probe { "yes" } else { "no" },
            if write_ok { "yes" } else { "no" },
            readback,
            src.display()
        );
    }
}

fn walkdir(p: &std::path::Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if p.is_file() {
        out.push(p.to_path_buf());
        return out;
    }
    if let Ok(entries) = std::fs::read_dir(p) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                out.extend(walkdir(&path));
            } else {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}
