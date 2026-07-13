//! CSS validator used by the center during hot reloads

use std::env;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use anyhow::{Context, Result};
use gtk::prelude::*;
use gtk::CssProvider;

const USAGE: &str = "usage: css-check <css-path> [css-path...]";

// Keeps argument validation isolated from GTK initialization for unit testing
fn parse_args<I, S>(args: I) -> Option<Vec<PathBuf>>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let paths: Vec<PathBuf> = args
        .into_iter()
        .map(Into::into)
        .map(PathBuf::from)
        .collect();
    if paths.is_empty() {
        None
    } else {
        Some(paths)
    }
}

// Splits existing and missing paths to keep file checks testable without GTK
fn partition_existing_paths(
    paths: impl IntoIterator<Item = PathBuf>,
) -> (Vec<PathBuf>, Vec<PathBuf>) {
    // Iterator::partition keeps split logic localized while preserving ownership
    // so extra clones are avoided in this test-friendly helper
    paths.into_iter().partition(|path| path.exists())
}

fn main() -> Result<()> {
    gtk::init().context("initialize gtk")?;

    let Some(args) = parse_args(env::args().skip(1)) else {
        eprintln!("{USAGE}");
        std::process::exit(2);
    };

    let error_count = Arc::new(AtomicUsize::new(0));
    let provider = CssProvider::new();
    let error_count_clone = error_count.clone();
    provider.connect_parsing_error(move |_provider, section, error| {
        error_count_clone.fetch_add(1, Ordering::Relaxed);
        let location = section.start_location();
        let file = section
            .file()
            .and_then(|file| file.path())
            .map_or_else(|| "<data>".to_string(), |path| path.display().to_string());
        eprintln!(
            "css error: {}:{}:{}: {}",
            file,
            location.lines() + 1,
            location.line_chars() + 1,
            error.message()
        );
    });

    let (existing, missing) = partition_existing_paths(args);
    for path in missing {
        error_count.fetch_add(1, Ordering::Relaxed);
        eprintln!("css error: {}: file not found", path.display());
    }
    for path in existing {
        provider.load_from_path(&path);
    }

    if error_count.load(Ordering::Relaxed) > 0 {
        std::process::exit(1);
    }

    Ok(())
}

#[cfg(test)]
#[path = "tests/css_check.rs"]
mod tests;
