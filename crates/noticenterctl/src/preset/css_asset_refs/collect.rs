use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use unixnotis_core::util;

use super::parse::{collect_import_values, collect_url_values, CssImportReference};
use super::{
    asset_path_reason, classify_file_url, has_css_extension, read_css_path_text_bounded,
    read_css_text, ExternalCssAssetRef, FileUrlClassification,
};
use crate::preset::archive::BundleFile;
use crate::preset::config_root::PresetFileSource;
use crate::preset::config_root::SecureFileCapture;
use crate::preset::pathing::normalize_lexical_path;

const MAX_COLLECTED_CSS_REFERENCES: usize = 4_096;

pub(in crate::preset) fn collect_external_css_asset_refs_from_bundle(
    config_dir: &Path,
    files: &[BundleFile],
) -> Result<Vec<ExternalCssAssetRef>> {
    let mut refs = Vec::new();

    // Bundle files are already in memory, so import can warn before any write happens
    for file in files {
        if !has_css_extension(&file.relative_path) {
            continue;
        }
        // Bundle CSS paths are rebuilt under the target config root so the warning matches import
        let css_path = config_dir.join(&file.relative_path);
        let css_text = String::from_utf8_lossy(&file.contents);
        extend_external_refs(
            &mut refs,
            collect_external_refs_from_text(config_dir, &css_path, css_text.as_ref())?,
        )?;
    }

    Ok(refs)
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
        extend_external_refs(
            &mut refs,
            collect_external_refs_from_text(config_dir, &file.source_path, &css_text)?,
        )?;
    }

    Ok(refs)
}

pub fn collect_external_css_asset_refs_from_paths(
    config_dir: &Path,
    css_paths: &[PathBuf],
) -> Result<Vec<ExternalCssAssetRef>> {
    let mut refs = Vec::new();

    for css_path in css_paths {
        // css-check reads the on-disk stylesheet directly because no bundle exists in that flow
        let css_text = read_css_path_text_bounded(css_path)?;
        extend_external_refs(
            &mut refs,
            collect_external_refs_from_text(config_dir, css_path, &css_text)?,
        )?;
    }

    Ok(refs)
}

pub(in crate::preset) fn collect_local_css_asset_paths_from_captures(
    config_dir: &Path,
    css_paths: &[PathBuf],
    captures: &std::collections::BTreeMap<PathBuf, SecureFileCapture>,
) -> Result<Vec<PathBuf>> {
    let normalized_root = normalize_lexical_path(config_dir);
    let mut paths = Vec::new();

    for css_path in css_paths {
        let relative = css_path
            .strip_prefix(config_dir)
            .with_context(|| format!("make stylesheet relative: {}", css_path.display()))?;
        let capture = captures.get(relative).ok_or_else(|| {
            anyhow::anyhow!(
                "active stylesheet was not securely captured: {}",
                css_path.display()
            )
        })?;
        // Export scans the descriptor-captured bytes instead of reopening a mutable live path
        let css_text = std::str::from_utf8(&capture.contents)
            .with_context(|| format!("stylesheet is not valid UTF-8: {}", css_path.display()))?;
        collect_local_paths_from_text(
            config_dir,
            css_path,
            css_text,
            &normalized_root,
            &mut paths,
        )?;
    }

    paths.sort();
    paths.dedup();
    Ok(paths)
}

fn collect_local_paths_from_text(
    config_dir: &Path,
    css_path: &Path,
    css_text: &str,
    normalized_root: &Path,
    paths: &mut Vec<PathBuf>,
) -> Result<()> {
    // The tokenizer skips real comments while preserving comment markers inside quoted paths
    for reference in collect_url_values(css_text)? {
        if reference.ambiguous {
            continue;
        }
        collect_local_path(
            config_dir,
            css_path,
            &reference.value,
            normalized_root,
            paths,
        )?;
    }
    for import_ref in collect_import_values(css_text)? {
        // Quoted imports are local file dependencies just like url(...) references
        let CssImportReference::Target(asset_ref) = import_ref else {
            continue;
        };
        collect_local_path(config_dir, css_path, &asset_ref, normalized_root, paths)?;
    }
    Ok(())
}

fn collect_local_path(
    config_dir: &Path,
    css_path: &Path,
    asset_ref: &str,
    normalized_root: &Path,
    paths: &mut Vec<PathBuf>,
) -> Result<()> {
    let trimmed = asset_ref.trim();
    let lowered = trimmed.to_ascii_lowercase();
    if trimmed.is_empty()
        || lowered.starts_with("data:")
        || lowered.starts_with("http://")
        || lowered.starts_with("https://")
    {
        return Ok(());
    }

    let candidate = match classify_file_url(trimmed) {
        FileUrlClassification::Local(path) => path,
        FileUrlClassification::NonLocalAuthority | FileUrlClassification::Malformed => {
            return Ok(())
        }
        FileUrlClassification::NotFileUrl => {
            let expanded = PathBuf::from(util::expand_tilde(trimmed).into_owned());
            if expanded.is_absolute() {
                expanded
            } else {
                css_path.parent().unwrap_or(config_dir).join(expanded)
            }
        }
    };
    let normalized = normalize_lexical_path(&candidate);
    if let Ok(relative) = normalized.strip_prefix(normalized_root) {
        if paths.len() >= MAX_COLLECTED_CSS_REFERENCES {
            // Total limits prevent many small stylesheets from bypassing the per-file parser cap
            anyhow::bail!(
                "CSS files contain more than {MAX_COLLECTED_CSS_REFERENCES} local references"
            );
        }
        // The selected-file collector performs the final filesystem safety checks
        paths.push(relative.to_path_buf());
    }
    Ok(())
}

fn extend_external_refs(
    refs: &mut Vec<ExternalCssAssetRef>,
    additional: Vec<ExternalCssAssetRef>,
) -> Result<()> {
    // One bundle-wide bound stops many small files from multiplying prompt memory
    if refs.len().saturating_add(additional.len()) > MAX_COLLECTED_CSS_REFERENCES {
        anyhow::bail!(
            "CSS files contain more than {MAX_COLLECTED_CSS_REFERENCES} external references"
        );
    }
    refs.extend(additional);
    Ok(())
}

fn collect_external_refs_from_text(
    config_dir: &Path,
    css_path: &Path,
    css_text: &str,
) -> Result<Vec<ExternalCssAssetRef>> {
    let mut refs = Vec::new();
    // Scanner state ignores real comments without corrupting quoted `/*` and `*/` bytes
    for reference in collect_url_values(css_text)? {
        let reason = if reference.ambiguous {
            Some("unrecognized CSS url syntax".to_string())
        } else {
            classify_external_asset_ref(config_dir, css_path, &reference.value)
        };
        if let Some(reason) = reason {
            refs.push(ExternalCssAssetRef {
                css_file: css_path.to_path_buf(),
                asset_ref: reference.value,
                reason,
            });
        }
    }
    for import_ref in collect_import_values(css_text)? {
        // Ambiguous imports remain visible instead of being guessed as safe relative paths
        let (asset_ref, reason) = match import_ref {
            CssImportReference::Target(asset_ref) => {
                let Some(reason) = classify_external_asset_ref(config_dir, css_path, &asset_ref)
                else {
                    continue;
                };
                (asset_ref, reason)
            }
            CssImportReference::Ambiguous => (
                "@import".to_string(),
                "unrecognized CSS import syntax".to_string(),
            ),
        };
        refs.push(ExternalCssAssetRef {
            css_file: css_path.to_path_buf(),
            asset_ref,
            reason,
        });
    }

    Ok(refs)
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
    match classify_file_url(trimmed) {
        FileUrlClassification::Local(path) => {
            // Local file URLs are decoded before the same containment check as plain paths
            return asset_path_reason(config_dir, &path);
        }
        FileUrlClassification::NonLocalAuthority => {
            return Some("non-local file URL".to_string());
        }
        FileUrlClassification::Malformed => return Some("malformed file URL".to_string()),
        FileUrlClassification::NotFileUrl => {}
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

#[cfg(test)]
#[path = "tests/collect.rs"]
mod tests;
