//! CSS file filtering, decoding, and local path classification

use std::fs;
use std::io::Read;
use std::path::Path;

use anyhow::{Context, Result};
use rustix::fs::{open, Mode, OFlags};

use super::super::config_root::PresetFileSource;
use super::super::pathing::normalize_lexical_path;

const MAX_CSS_FILE_BYTES: u64 = 16_777_216;

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

pub fn read_css_path_text_bounded(path: &Path) -> Result<String> {
    let (bytes, _metadata) = read_css_file_bounded(path)?;
    String::from_utf8(bytes)
        .with_context(|| format!("CSS file is not valid UTF-8: {}", path.display()))
}

pub fn read_css_file_bounded(path: &Path) -> Result<(Vec<u8>, fs::Metadata)> {
    // Nonblocking opens make special files fail safely instead of freezing diagnostics
    let fd = open(path, OFlags::CLOEXEC.union(OFlags::NONBLOCK), Mode::empty())
        .with_context(|| format!("open CSS file {}", path.display()))?;
    let mut file = fs::File::from(fd);
    let metadata = file
        .metadata()
        .with_context(|| format!("inspect CSS file {}", path.display()))?;
    if !metadata.is_file() {
        anyhow::bail!("CSS path is not a regular file: {}", path.display());
    }
    if metadata.len() > MAX_CSS_FILE_BYTES {
        anyhow::bail!(
            "CSS file exceeds {MAX_CSS_FILE_BYTES} bytes: {}",
            path.display()
        );
    }

    // The metadata length is only a reserve hint because the file may still change while read
    let reserve =
        usize::try_from(metadata.len()).context("CSS file size does not fit in memory")?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(reserve)
        .context("reserve memory for CSS file")?;
    file.by_ref()
        .take(MAX_CSS_FILE_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)
        .with_context(|| format!("read CSS file {}", path.display()))?;
    let size = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if size.cmp(&MAX_CSS_FILE_BYTES).is_gt() {
        anyhow::bail!(
            "CSS file grew beyond {MAX_CSS_FILE_BYTES} bytes: {}",
            path.display()
        );
    }

    Ok((bytes, metadata))
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
