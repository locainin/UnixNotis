//! CSS asset reference checks shared by preset flows and css-check
//!
//! This module keeps CSS asset scanning and bundle-only rewrites in one tree
//! so preset import, export, inspect, and css-check all read the same rules

#[path = "css_asset_refs/collect.rs"]
mod collect;
#[path = "css_asset_refs/parse.rs"]
mod parse;
#[path = "css_asset_refs/rewrite.rs"]
mod rewrite;
#[cfg(test)]
#[path = "css_asset_refs/tests.rs"]
mod tests;

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use super::config_root::PresetFileSource;
use super::pathing::normalize_lexical_path;

pub(crate) use self::collect::collect_external_css_asset_refs_from_paths;
pub(super) use self::collect::{
    collect_external_css_asset_refs_from_bundle, collect_external_css_asset_refs_from_collected,
};
pub(super) use self::rewrite::rewrite_host_specific_css_asset_refs_in_sources;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExternalCssAssetRef {
    // CSS file that carried the outside asset reference
    pub(crate) css_file: PathBuf,
    // Raw url(...) payload as written in the stylesheet
    pub(crate) asset_ref: String,
    // Short reason shown back to the caller
    pub(crate) reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HostSpecificCssAssetRef {
    // CSS file that carried the host-local config path
    pub(crate) css_file: PathBuf,
    // Raw url(...) payload as written in the stylesheet
    pub(crate) asset_ref: String,
    // Replacement path written into the bundled stylesheet
    pub(crate) rewritten_ref: String,
}

fn has_css_extension(path: &Path) -> bool {
    // CSS-only filtering keeps later URL parsing away from binary assets and config files
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("css"))
        .unwrap_or(false)
}

fn read_css_text(file: &PresetFileSource) -> Result<String> {
    if let Some(contents) = &file.contents_override {
        // Export can patch stylesheet bytes in memory without touching the live config tree
        return String::from_utf8(contents.clone())
            .with_context(|| format!("decode css override {}", file.relative_path.display()));
    }

    std::fs::read_to_string(&file.source_path)
        .with_context(|| format!("read css file {}", file.source_path.display()))
}

fn local_file_url_path(value: &str) -> Option<PathBuf> {
    // Only local file URLs are treated as path leaks here
    let path = value.strip_prefix("file://")?;
    // file://localhost/path is still a local file URL and should be treated the same
    let path = path.strip_prefix("localhost/").unwrap_or(path);
    if !path.starts_with('/') {
        return None;
    }
    Some(PathBuf::from(path))
}

fn asset_path_reason(config_dir: &Path, candidate: &Path) -> Option<String> {
    // The check stays lexical so missing files and future import targets are handled the same way
    let normalized_root = normalize_lexical_path(config_dir);
    let normalized_candidate = normalize_lexical_path(candidate);
    if normalized_candidate.starts_with(&normalized_root) {
        return None;
    }
    Some("local path points outside the config root".to_string())
}
