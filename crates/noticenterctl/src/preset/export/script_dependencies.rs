//! Config-relative script dependency discovery for preset export
//!
//! This intentionally recognizes only direct shell source statements
//! Broad shell evaluation would execute user content and could recapture unrelated files

use std::collections::{BTreeSet, VecDeque};
use std::path::{Component, Path, PathBuf};

use anyhow::{anyhow, Context, Result};

// Source scanning is only dependency metadata work, so a small cap prevents an oversized command
// file from consuming memory before the normal preset file limits are applied
pub(super) const MAX_SCANNED_SCRIPT_BYTES: u64 = 1024 * 1024;

pub(super) fn collect_script_dependency_closure(
    config_dir: &Path,
    entry_scripts: &[PathBuf],
) -> Result<Vec<PathBuf>> {
    let mut discovered = BTreeSet::new();
    let mut pending = VecDeque::new();

    for entry in entry_scripts {
        // Config command resolution has already constrained entries to the config root
        // Normalizing again keeps this helper safe when called independently by tests
        let Some(entry) = normalize_relative_path(entry) else {
            continue;
        };
        if discovered.insert(entry.clone()) {
            pending.push_back(entry);
        }
    }

    while let Some(script_relative) = pending.pop_front() {
        let script_path = config_dir.join(&script_relative);
        let metadata = match std::fs::symlink_metadata(&script_path) {
            Ok(metadata) if metadata.file_type().is_file() => metadata,
            // The collector later reports or skips non-regular entry paths using its shared policy
            Ok(_) | Err(_) => continue,
        };
        if metadata.len() > MAX_SCANNED_SCRIPT_BYTES {
            // Large command payloads are not treated as shell source because dependency parsing is bounded
            continue;
        }

        let bytes = std::fs::read(&script_path)
            .with_context(|| format!("read script dependency source {}", script_path.display()))?;
        let Ok(contents) = std::str::from_utf8(&bytes) else {
            // Binary command helpers cannot contain portable shell source statements
            continue;
        };

        for operand in source_operands(contents) {
            let Some(dependency) = resolve_source_operand(&script_relative, &operand) else {
                // Dynamic variables and absolute system libraries are runtime concerns, not bundle paths
                continue;
            };
            let dependency_path = config_dir.join(&dependency);
            let dependency_metadata =
                std::fs::symlink_metadata(&dependency_path).with_context(|| {
                    format!(
                        "script {} sources missing preset dependency {}",
                        script_relative.display(),
                        dependency.display()
                    )
                })?;
            if !dependency_metadata.file_type().is_file() {
                return Err(anyhow!(
                    "script {} sources non-regular preset dependency {}",
                    script_relative.display(),
                    dependency.display()
                ));
            }
            if discovered.insert(dependency.clone()) {
                // Newly found helpers are scanned too because shell libraries can source another local file
                pending.push_back(dependency);
            }
        }
    }

    Ok(discovered.into_iter().collect())
}

fn source_operands(contents: &str) -> impl Iterator<Item = String> + '_ {
    contents.lines().filter_map(|line| {
        let trimmed = line.trim_start();
        // Token matching naturally rejects blank lines and comments without a second parsing rule
        let words = shell_words::split(trimmed).ok()?;
        match words.as_slice() {
            [command, operand, ..] if command == "." || command == "source" => {
                Some(operand.clone())
            }
            _ => None,
        }
    })
}

fn resolve_source_operand(script_relative: &Path, operand: &str) -> Option<PathBuf> {
    let script_parent = script_relative.parent().unwrap_or_else(|| Path::new(""));
    let relative = if let Some(value) = operand.strip_prefix("$script_dir/") {
        script_parent.join(value)
    } else if let Some(value) = operand.strip_prefix("${script_dir}/") {
        script_parent.join(value)
    } else {
        // Any remaining expansion requires a shell and must never be guessed during export
        if operand.contains('$') || operand.contains('`') || Path::new(operand).is_absolute() {
            return None;
        }
        PathBuf::from(operand)
    };
    normalize_relative_path(&relative)
}

fn normalize_relative_path(path: &Path) -> Option<PathBuf> {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(value) => normalized.push(value),
            Component::CurDir => {}
            // Parent, root, and platform prefixes could leave the shared config tree
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    (!normalized.as_os_str().is_empty()).then_some(normalized)
}
