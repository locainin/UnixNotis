use std::path::{Component, Path, PathBuf};

use anyhow::Result;
use unixnotis_core::util;

use super::parse::collect_url_spans;
use super::{
    has_css_extension, local_file_url_path, read_css_text, HostSpecificCssAssetRef,
};
use crate::preset::config_root::PresetFileSource;
use crate::preset::pathing::normalize_lexical_path;

pub(in crate::preset) fn rewrite_host_specific_css_asset_refs_in_sources(
    config_dir: &Path,
    files: &mut [PresetFileSource],
) -> Result<Vec<HostSpecificCssAssetRef>> {
    let mut rewrites = Vec::new();

    for file in files {
        if !has_css_extension(&file.relative_path) {
            continue;
        }

        // Export rewrites the effective stylesheet text, not only the on-disk source bytes
        let css_text = read_css_text(file)?;
        let (rewritten_text, file_rewrites) =
            rewrite_host_specific_refs_in_text(config_dir, &file.source_path, &css_text);
        if file_rewrites.is_empty() {
            continue;
        }

        // Export keeps the rewrite in memory so the live stylesheet stays untouched
        file.size = rewritten_text.len() as u64;
        file.contents_override = Some(rewritten_text.into_bytes());
        rewrites.extend(file_rewrites);
    }

    Ok(rewrites)
}

fn rewrite_host_specific_refs_in_text(
    config_dir: &Path,
    css_path: &Path,
    css_text: &str,
) -> (String, Vec<HostSpecificCssAssetRef>) {
    let mut rewritten = String::with_capacity(css_text.len());
    let mut rewrites = Vec::new();
    let mut last_index = 0usize;

    for span in collect_url_spans(css_text) {
        // Everything before the current url(...) payload is copied through unchanged
        rewritten.push_str(&css_text[last_index..span.value_start]);

        if let Some(rewritten_ref) =
            rewrite_host_specific_asset_ref(config_dir, css_path, &span.value)
        {
            rewritten.push_str(&rewritten_ref);
            rewrites.push(HostSpecificCssAssetRef {
                css_file: css_path.to_path_buf(),
                asset_ref: span.value,
                rewritten_ref,
            });
        } else {
            rewritten.push_str(&span.value);
        }

        last_index = span.value_end;
    }

    rewritten.push_str(&css_text[last_index..]);
    (rewritten, rewrites)
}

fn rewrite_host_specific_asset_ref(
    config_dir: &Path,
    css_path: &Path,
    asset_ref: &str,
) -> Option<String> {
    let trimmed = asset_ref.trim();
    if trimmed.is_empty() {
        return None;
    }

    let asset_path = if let Some(path) = local_file_url_path(trimmed) {
        // file:/// refs already point at one concrete path, so they can be checked directly
        path
    } else {
        let expanded = PathBuf::from(util::expand_tilde(trimmed).into_owned());
        if !expanded.is_absolute() {
            // Only host-local absolute paths are rewritten here
            return None;
        }
        expanded
    };

    let normalized_root = normalize_lexical_path(config_dir);
    let normalized_asset = normalize_lexical_path(&asset_path);
    let relative_asset = normalized_asset.strip_prefix(&normalized_root).ok()?;

    // Rewritten asset paths stay relative to the stylesheet so imports work on any machine
    let css_base_dir = css_path.parent().unwrap_or(config_dir);
    let normalized_css_base = normalize_lexical_path(css_base_dir);
    Some(relative_css_path(
        &normalized_css_base,
        &normalized_root.join(relative_asset),
    ))
}

fn relative_css_path(base_dir: &Path, target_path: &Path) -> String {
    let base_parts = base_dir
        .components()
        .filter_map(normal_component)
        .collect::<Vec<_>>();
    let target_parts = target_path
        .components()
        .filter_map(normal_component)
        .collect::<Vec<_>>();

    let mut shared = 0usize;
    while shared < base_parts.len()
        && shared < target_parts.len()
        && base_parts[shared] == target_parts[shared]
    {
        // Shared leading path segments are dropped before building the relative path
        shared += 1;
    }

    let mut relative = PathBuf::new();
    for _ in shared..base_parts.len() {
        // Every extra base segment needs one `..` to walk back out of that folder
        relative.push("..");
    }
    for part in &target_parts[shared..] {
        // The remaining target path is then appended in order
        relative.push(part);
    }

    format_css_relative_path(&relative)
}

fn normal_component(component: Component<'_>) -> Option<String> {
    match component {
        Component::Normal(part) => Some(part.to_string_lossy().to_string()),
        _ => None,
    }
}

fn format_css_relative_path(path: &Path) -> String {
    // CSS URLs need a slash-joined relative string, not an OS-dependent display path
    path.components()
        .filter_map(|component| match component {
            Component::ParentDir => Some("..".to_string()),
            Component::Normal(part) => Some(part.to_string_lossy().to_string()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}
