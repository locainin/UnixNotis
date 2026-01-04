//! CSS validator used by the center during hot reloads.

use std::env;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use anyhow::{Context, Result};
use gtk::prelude::*;
use gtk::CssProvider;

fn main() -> Result<()> {
    gtk::init().context("initialize gtk")?;

    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: css-check <css-path> [css-path...]");
        std::process::exit(2);
    }

    let error_count = Arc::new(AtomicUsize::new(0));
    let provider = CssProvider::new();
    let error_count_clone = error_count.clone();
    provider.connect_parsing_error(move |_provider, section, error| {
        error_count_clone.fetch_add(1, Ordering::Relaxed);
        let location = section.start_location();
        let file = section
            .file()
            .and_then(|file| file.path())
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "<data>".to_string());
        eprintln!(
            "css error: {}:{}:{}: {}",
            file,
            location.lines() + 1,
            location.line_chars() + 1,
            error.message()
        );
    });

    for arg in args {
        let path = PathBuf::from(arg);
        if !path.exists() {
            error_count.fetch_add(1, Ordering::Relaxed);
            eprintln!("css error: {}: file not found", path.display());
            continue;
        }
        provider.load_from_path(&path);
    }

    if error_count.load(Ordering::Relaxed) > 0 {
        std::process::exit(1);
    }

    Ok(())
}
