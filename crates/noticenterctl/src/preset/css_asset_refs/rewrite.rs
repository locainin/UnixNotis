use std::path::{Path, PathBuf};

use anyhow::Result;
use unixnotis_core::util;
use url::Url;

use super::parse::collect_url_spans;
use super::{
    classify_file_url, has_css_extension, read_css_text, FileUrlClassification,
    HostSpecificCssAssetRef,
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
            rewrite_host_specific_refs_in_text(config_dir, &file.source_path, &css_text)?;
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
) -> Result<(String, Vec<HostSpecificCssAssetRef>)> {
    let mut rewritten = String::with_capacity(css_text.len());
    let mut rewrites = Vec::new();
    let mut last_index = 0usize;

    for span in collect_url_spans(css_text)? {
        // Everything before the current url(...) payload is copied through unchanged
        rewritten.push_str(&css_text[last_index..span.value_start]);

        if span.ambiguous {
            rewritten.push_str(&span.value);
        } else if let Some(rewritten_ref) =
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
    Ok((rewritten, rewrites))
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

    let asset_path = match classify_file_url(trimmed) {
        FileUrlClassification::Local(path) => path,
        FileUrlClassification::NonLocalAuthority | FileUrlClassification::Malformed => return None,
        FileUrlClassification::NotFileUrl => {
            let expanded = PathBuf::from(util::expand_tilde(trimmed).into_owned());
            if !expanded.is_absolute() {
                // Only host-local absolute paths are rewritten here
                return None;
            }
            expanded
        }
    };

    let normalized_root = normalize_lexical_path(config_dir);
    let normalized_asset = normalize_lexical_path(&asset_path);
    let relative_asset = normalized_asset.strip_prefix(&normalized_root).ok()?;

    // Rewritten asset paths stay relative to the stylesheet so imports work on any machine
    let css_base_dir = css_path.parent().unwrap_or(config_dir);
    let normalized_css_base = normalize_lexical_path(css_base_dir);
    relative_css_url(&normalized_css_base, &normalized_root.join(relative_asset))
}

fn relative_css_url(base_dir: &Path, target_path: &Path) -> Option<String> {
    // Directory URL conversion keeps the trailing slash needed for correct relative resolution
    let base_url = Url::from_directory_path(base_dir).ok()?;
    // File URL conversion serializes spaces and CSS-significant bytes as percent escapes
    let target_url = Url::from_file_path(target_path).ok()?;
    base_url
        .make_relative(&target_url)
        .map(|reference| encode_css_url_token_delimiters(&reference))
}

fn encode_css_url_token_delimiters(reference: &str) -> String {
    let mut encoded = String::with_capacity(reference.len());
    for character in reference.chars() {
        let escape = match character {
            // These bytes can quote, terminate, escape, or invalidate an unquoted CSS URL token
            '"' => Some("%22"),
            '\'' => Some("%27"),
            '(' => Some("%28"),
            ')' => Some("%29"),
            '\\' => Some("%5C"),
            '\t' => Some("%09"),
            '\n' => Some("%0A"),
            '\r' => Some("%0D"),
            '\u{000C}' => Some("%0C"),
            _ => None,
        };
        if let Some(escape) = escape {
            encoded.push_str(escape);
        } else {
            encoded.push(character);
        }
    }
    encoded
}

#[cfg(test)]
#[path = "tests/rewrite.rs"]
mod tests;
