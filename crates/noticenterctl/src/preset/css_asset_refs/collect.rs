use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use unixnotis_core::util;

use super::parse::{collect_url_values, strip_css_comments};
use super::{
    asset_path_reason, has_css_extension, local_file_url_path, read_css_text, ExternalCssAssetRef,
};
use crate::preset::archive::BundleFile;
use crate::preset::config_root::PresetFileSource;
use crate::preset::pathing::normalize_lexical_path;

pub(in crate::preset) fn collect_external_css_asset_refs_from_bundle(
    config_dir: &Path,
    files: &[BundleFile],
) -> Vec<ExternalCssAssetRef> {
    let mut refs = Vec::new();

    // Bundle files are already in memory, so import can warn before any write happens
    for file in files {
        if !has_css_extension(&file.relative_path) {
            continue;
        }
        // Bundle CSS paths are rebuilt under the target config root so the warning matches import
        let css_path = config_dir.join(&file.relative_path);
        let css_text = String::from_utf8_lossy(&file.contents);
        refs.extend(collect_external_refs_from_text(
            config_dir,
            &css_path,
            css_text.as_ref(),
        ));
    }

    refs
}

pub(in crate::preset) fn collect_external_css_asset_refs_from_collected(
    config_dir: &Path,
    files: &[PresetFileSource],
) -> Result<Vec<ExternalCssAssetRef>> {
    let mut refs = Vec::new();

    // Export may already have in-memory overrides, so this scans the exact bundle view
    for file in files {
        if !has_css_extension(&file.relative_path) {
            continue;
        }

        // Overrides matter here because export may already have rewritten the bundled stylesheet
        let css_text = read_css_text(file)?;
        refs.extend(collect_external_refs_from_text(
            config_dir,
            &file.source_path,
            &css_text,
        ));
    }

    Ok(refs)
}

pub(crate) fn collect_external_css_asset_refs_from_paths(
    config_dir: &Path,
    css_paths: &[PathBuf],
) -> Result<Vec<ExternalCssAssetRef>> {
    let mut refs = Vec::new();

    for css_path in css_paths {
        // css-check reads the on-disk stylesheet directly because no bundle exists in that flow
        let css_text = std::fs::read_to_string(css_path)
            .with_context(|| format!("read css file {}", css_path.display()))?;
        refs.extend(collect_external_refs_from_text(
            config_dir, css_path, &css_text,
        ));
    }

    Ok(refs)
}

fn collect_external_refs_from_text(
    config_dir: &Path,
    css_path: &Path,
    css_text: &str,
) -> Vec<ExternalCssAssetRef> {
    let mut refs = Vec::new();
    let stripped = strip_css_comments(css_text);

    // URL extraction happens after comments are stripped so dead example paths do not warn
    for asset_ref in collect_url_values(&stripped) {
        if let Some(reason) = classify_external_asset_ref(config_dir, css_path, &asset_ref) {
            refs.push(ExternalCssAssetRef {
                css_file: css_path.to_path_buf(),
                asset_ref,
                reason,
            });
        }
    }

    refs
}

fn classify_external_asset_ref(
    config_dir: &Path,
    css_path: &Path,
    asset_ref: &str,
) -> Option<String> {
    let trimmed = asset_ref.trim();
    if trimmed.is_empty() {
        return None;
    }

    let lowered = trimmed.to_ascii_lowercase();
    if lowered.starts_with("data:") {
        // Embedded data stays self-contained inside the stylesheet
        return None;
    }
    if lowered.starts_with("http://") || lowered.starts_with("https://") {
        // Remote assets are not portable bundle content and should stay visible in warnings
        return Some("remote url".to_string());
    }
    if let Some(path) = local_file_url_path(trimmed) {
        // file:/// paths are already absolute, so they can be checked against the config root directly
        return asset_path_reason(config_dir, &path);
    }

    let expanded = PathBuf::from(util::expand_tilde(trimmed).into_owned());
    if expanded.is_absolute() {
        // Plain absolute paths leak the local machine layout the same way file:/// paths do
        return asset_path_reason(config_dir, &expanded);
    }

    // Relative refs are anchored to the stylesheet location, not the config root itself
    let base_dir = css_path.parent().unwrap_or(config_dir);
    let resolved = normalize_lexical_path(&base_dir.join(expanded));
    let normalized_root = normalize_lexical_path(config_dir);
    if !resolved.starts_with(&normalized_root) {
        return Some("relative path leaves the config root".to_string());
    }

    None
}
