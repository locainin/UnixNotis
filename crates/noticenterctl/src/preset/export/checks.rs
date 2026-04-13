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
    // Exact text found in the script before rewrite
    pub(super) needle: String,
    // Replacement shown in warning output
    pub(super) rewritten_to: String,
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
        let source_text = read_script_text(file)?;
        let mut rewritten = source_text.clone();
        let mut matched_needle = None::<String>;

        for signature in &signatures {
            // Replace every known config-root form with a portable shell-safe value
            if rewritten.contains(&signature.needle) {
                matched_needle = Some(signature.needle.clone());
                rewritten = rewritten.replace(&signature.needle, &signature.rewrite_to);
            }
        }

        if let Some(needle) = matched_needle {
            // Export writes only override bytes and never mutates the live script on disk
            file.size = rewritten.len() as u64;
            file.contents_override = Some(rewritten.into_bytes());
            leaks.push(HostSpecificScriptLeak {
                script_path: file.relative_path.clone(),
                needle,
                rewritten_to: home_fallback_root.clone(),
            });
        }
    }

    Ok(leaks)
}

pub(super) fn clear_script_overrides(
    files: &mut [PresetFileSource],
    leaks: &[HostSpecificScriptLeak],
) {
    for leak in leaks {
        // Declining rewrite drops temporary bytes so the original file is archived as-is
        if let Some(file) = files
            .iter_mut()
            .find(|file| file.relative_path == leak.script_path)
        {
            file.contents_override = None;
            if let Ok(metadata) = std::fs::metadata(&file.source_path) {
                file.size = metadata.len();
            }
        }
    }
}

fn read_script_text(file: &PresetFileSource) -> Result<String> {
    // Prefer in-memory override bytes when another rewrite already touched this file
    if let Some(contents) = &file.contents_override {
        return Ok(String::from_utf8_lossy(contents).into_owned());
    }

    let bytes = std::fs::read(&file.source_path).map_err(|err| {
        anyhow!(
            "read script file for host-path checks {}: {}",
            file.source_path.display(),
            err
        )
    })?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
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
