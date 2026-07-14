//! Export-only validation helpers
//!
//! These checks are specific to building a shareable preset from the live tree

use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};

use crate::preset::config_root::PresetFileSource;
use crate::preset::pathing::normalize_lexical_path;

pub(super) fn validate_theme_paths_stay_in_root(
    config_dir: &Path,
    theme_paths: &[(&'static str, &Path)],
) -> Result<()> {
    let normalized_root = normalize_lexical_path(config_dir);

    // A shareable preset should not depend on files stored outside the config root
    for (slot_name, path) in theme_paths {
        let normalized_path = normalize_lexical_path(path);
        if !normalized_path.starts_with(&normalized_root) {
            return Err(anyhow!(
                "preset export requires {} to live under the config root: {}",
                slot_name,
                path.display()
            ));
        }
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct HostSpecificScriptLeak {
    // Relative script path inside the bundled preset
    pub(super) script_path: PathBuf,
    // Every matched path form is kept so the warning output does not underreport
    pub(super) needles: Vec<String>,
    // Replacement shown in warning output
    pub(super) rewritten_to: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct FileOverrideSnapshot {
    // Relative path is the stable key inside the collected export set
    pub(super) relative_path: PathBuf,
    // Size and bytes need to roll back together
    pub(super) size: u64,
    pub(super) contents_override: Option<Vec<u8>>,
}

pub(super) fn capture_file_overrides(files: &[PresetFileSource]) -> Vec<FileOverrideSnapshot> {
    files
        .iter()
        .map(|file| FileOverrideSnapshot {
            relative_path: file.relative_path.clone(),
            size: file.size,
            contents_override: file.contents_override.clone(),
        })
        .collect()
}

pub(super) fn restore_file_overrides(
    files: &mut [PresetFileSource],
    snapshots: &[FileOverrideSnapshot],
) {
    for snapshot in snapshots {
        // Restore the staged export view exactly as it was before any rewrite attempt
        if let Some(file) = files
            .iter_mut()
            .find(|file| file.relative_path == snapshot.relative_path)
        {
            file.size = snapshot.size;
            file.contents_override = snapshot.contents_override.clone();
        }
    }
}

pub(super) fn rewrite_host_specific_script_paths_in_sources(
    config_dir: &Path,
    files: &mut [PresetFileSource],
) -> Result<Vec<HostSpecificScriptLeak>> {
    // Keep script rewrites username-safe and portable in shell scripts
    let home_fallback_root = build_home_fallback_root();
    let signatures = script_root_signatures(config_dir);

    let mut leaks = Vec::new();
    for file in files {
        // Only bundled scripts are scanned for host-local config roots
        if !is_script_path(file.relative_path.as_path()) {
            continue;
        }

        // Read pending override bytes first so multiple rewrites stay consistent
        let Some(source_text) = read_script_text(file)? else {
            // Binary or non-UTF-8 helpers under scripts/ stay untouched instead of being mangled
            continue;
        };
        let mut rewritten = source_text.clone();
        let mut matched_needles = Vec::new();

        for signature in &signatures {
            // Replace every known config-root form with a portable shell-safe value
            if rewritten.contains(&signature.needle) {
                matched_needles.push(signature.needle.clone());
                rewritten = rewritten.replace(&signature.needle, &signature.rewrite_to);
            }
        }

        if !matched_needles.is_empty() {
            // Export writes only override bytes and never mutates the live script on disk
            file.size = rewritten.len() as u64;
            file.contents_override = Some(rewritten.into_bytes());
            leaks.push(HostSpecificScriptLeak {
                script_path: file.relative_path.clone(),
                needles: matched_needles,
                rewritten_to: home_fallback_root.clone(),
            });
        }
    }

    Ok(leaks)
}

fn read_script_text(file: &PresetFileSource) -> Result<Option<String>> {
    // Prefer in-memory override bytes when another rewrite already touched this file
    if let Some(contents) = &file.contents_override {
        return Ok(decode_script_text(contents));
    }

    Ok(decode_script_text(&file.source_contents))
}

fn decode_script_text(bytes: &[u8]) -> Option<String> {
    // Exact UTF-8 is required here so binary helpers cannot be lossy-decoded and corrupted
    if bytes.contains(&0) {
        return None;
    }

    std::str::from_utf8(bytes).ok().map(str::to_string)
}

#[derive(Clone)]
struct ScriptRootSignature {
    needle: String,
    rewrite_to: String,
}

fn script_root_signatures(config_dir: &Path) -> Vec<ScriptRootSignature> {
    let normalized_root = normalize_lexical_path(config_dir);
    let root_text = normalized_root.to_string_lossy().to_string();
    let home_fallback_root = build_home_fallback_root();
    let home_fallback_file_url = format!("file://{home_fallback_root}");

    // Match plain paths and file URL forms so script rewriting is format-agnostic
    vec![
        ScriptRootSignature {
            needle: format!("file://localhost{root_text}"),
            rewrite_to: home_fallback_file_url.clone(),
        },
        ScriptRootSignature {
            needle: format!("file://{root_text}"),
            rewrite_to: home_fallback_file_url,
        },
        ScriptRootSignature {
            needle: root_text,
            rewrite_to: home_fallback_root,
        },
    ]
}

fn build_home_fallback_root() -> String {
    // Keep export portable across hosts with different usernames
    "${XDG_CONFIG_HOME:-$HOME/.config}/unixnotis".to_string()
}

fn is_script_path(relative_path: &Path) -> bool {
    // Only files under scripts/ are checked here
    matches!(
        relative_path.components().next(),
        Some(std::path::Component::Normal(first)) if first == "scripts"
    )
}

#[cfg(test)]
#[path = "checks/tests/cases.rs"]
mod tests;
