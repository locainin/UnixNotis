//! CSS validator used by the center during hot reloads.

use std::env;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use anyhow::{Context, Result};
use gtk::prelude::*;
use gtk::CssProvider;

const USAGE: &str = "usage: css-check <css-path> [css-path...]";

// Keeps argument validation isolated from GTK initialization for unit testing.
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

// Splits existing and missing paths to keep file checks testable without GTK.
fn partition_existing_paths(paths: impl IntoIterator<Item = PathBuf>) -> (Vec<PathBuf>, Vec<PathBuf>) {
    // Iterator::partition keeps split logic localized while preserving ownership
    // so extra clones are avoided in this test-friendly helper.
    paths.into_iter().partition(|path| path.exists())
}

fn main() -> Result<()> {
    gtk::init().context("initialize gtk")?;

    let args = match parse_args(env::args().skip(1)) {
        Some(paths) => paths,
        None => {
            eprintln!("{USAGE}");
            std::process::exit(2);
        }
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
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    // Creates unique temp file paths for isolation across test runs.
    fn unique_temp_path(label: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        let pid = std::process::id();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        path.push(format!("unixnotis-css-check-{}-{}-{}", pid, nanos, label));
        path
    }

    #[test]
    fn parse_args_returns_none_for_empty() {
        let result = parse_args(std::iter::empty::<String>());
        assert!(result.is_none());
    }

    #[test]
    fn parse_args_returns_paths_for_non_empty() {
        let result = parse_args(["a.css", "b.css"]).expect("paths should be present");
        assert_eq!(result, vec![PathBuf::from("a.css"), PathBuf::from("b.css")]);
    }

    #[test]
    fn partition_existing_paths_splits_missing_and_existing() {
        let existing_path = unique_temp_path("existing.css");
        fs::write(&existing_path, "body {}").expect("temp file write should succeed");
        let missing_path = unique_temp_path("missing.css");

        let (existing, missing) = partition_existing_paths(vec![existing_path.clone(), missing_path.clone()]);

        assert_eq!(existing, vec![existing_path.clone()]);
        assert_eq!(missing, vec![missing_path]);

        fs::remove_file(&existing_path).expect("temp file cleanup should succeed");
    }
}
