//! CSS file filtering, decoding, and local path classification

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use super::super::config_root::PresetFileSource;
use super::super::pathing::normalize_lexical_path;

pub(in crate::preset) fn has_css_extension(path: &Path) -> bool {
    // CSS-only filtering keeps later URL parsing away from binary assets and config files
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("css"))
}

pub(in crate::preset) fn read_css_text(file: &PresetFileSource) -> Result<String> {
    if let Some(contents) = &file.contents_override {
        // Export can patch stylesheet bytes in memory without touching the live config tree
        return String::from_utf8(contents.clone())
            .with_context(|| format!("decode css override {}", file.relative_path.display()));
    }

    String::from_utf8(file.source_contents.clone())
        .with_context(|| format!("decode css file {}", file.relative_path.display()))
}

pub(in crate::preset) fn local_file_url_path(value: &str) -> Option<PathBuf> {
    // Only local file URLs are treated as path leaks here
    let path = value.strip_prefix("file://")?;
    // file://localhost/path is still a local file URL and should be treated the same
    // Keep the leading slash because it is part of the absolute local path
    let path = path.strip_prefix("localhost").unwrap_or(path);
    if !path.starts_with('/') {
        return None;
    }
    Some(PathBuf::from(path))
}

pub(in crate::preset) fn asset_path_reason(config_dir: &Path, candidate: &Path) -> Option<String> {
    // The check stays lexical so missing files and future import targets are handled the same way
    let normalized_root = normalize_lexical_path(config_dir);
    let normalized_candidate = normalize_lexical_path(candidate);
    if normalized_candidate.starts_with(&normalized_root) {
        return None;
    }
    Some("local path points outside the config root".to_string())
}

#[cfg(test)]
#[path = "tests/paths.rs"]
mod tests;
