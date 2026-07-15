//! Config-relative script dependency discovery for preset export
//!
//! This intentionally recognizes only direct shell source statements
//! Broad shell evaluation would execute user content and could recapture unrelated files

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result};

use super::super::config_root::SecureFileCapture;
use super::super::filesystem::{open_secure_dir_all, read_relative_file_secure_bounded};

// Source scanning is only dependency metadata work, so a small cap prevents an oversized command
// file from consuming memory before the normal preset file limits are applied
pub(super) const MAX_SCANNED_SCRIPT_BYTES: u64 = 1024 * 1024;

pub(super) struct ScriptDependencyClosure {
    pub(super) paths: Vec<PathBuf>,
    pub(super) captures: BTreeMap<PathBuf, SecureFileCapture>,
}

pub(super) fn collect_script_dependency_closure(
    config_dir: &Path,
    entry_scripts: &[PathBuf],
) -> Result<ScriptDependencyClosure> {
    let root_fd = open_secure_dir_all(config_dir)
        .with_context(|| format!("open config directory {}", config_dir.display()))?;
    let mut discovered = BTreeSet::new();
    let mut pending = VecDeque::new();
    let mut captures = BTreeMap::new();

    for entry in entry_scripts {
        // Config command resolution has already constrained entries to the config root
        // Normalizing again keeps this helper safe when called independently by tests
        let Some(entry) = normalize_relative_path(entry) else {
            continue;
        };
        if discovered.insert(entry.clone()) {
            let (contents, mode) =
                read_relative_file_secure_bounded(&root_fd, &entry, MAX_SCANNED_SCRIPT_BYTES)
                    .with_context(|| {
                        format!(
                            "preset export cannot verify dependencies for script {}",
                            entry.display()
                        )
                    })?;
            captures.insert(entry.clone(), SecureFileCapture { contents, mode });
            pending.push_back(entry);
        }
    }

    while let Some(script_relative) = pending.pop_front() {
        let bytes = &captures
            .get(&script_relative)
            .expect("queued script capture must exist")
            .contents;
        let Ok(contents) = std::str::from_utf8(bytes) else {
            // Binary command helpers cannot contain portable shell source statements
            continue;
        };

        // Own the small operand list before captures grow so no map entry stays borrowed
        let operands = source_operands(contents).collect::<Vec<_>>();
        for operand in operands {
            let Some(dependency) = resolve_source_operand(&script_relative, &operand) else {
                // Dynamic variables and absolute system libraries are runtime concerns, not bundle paths
                continue;
            };
            if discovered.insert(dependency.clone()) {
                let (contents, mode) = read_relative_file_secure_bounded(
                    &root_fd,
                    &dependency,
                    MAX_SCANNED_SCRIPT_BYTES,
                )
                .with_context(|| {
                    format!(
                        "script {} sources missing, unsafe, or oversized preset dependency {}",
                        script_relative.display(),
                        dependency.display()
                    )
                })?;
                captures.insert(dependency.clone(), SecureFileCapture { contents, mode });
                // Newly found helpers are scanned too because shell libraries can source another local file
                pending.push_back(dependency);
            }
        }
    }

    Ok(ScriptDependencyClosure {
        paths: discovered.into_iter().collect(),
        captures,
    })
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
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(value) => parts.push(value.to_os_string()),
            Component::CurDir => {}
            // Parent traversal is safe only while a prior config-relative segment remains to pop
            Component::ParentDir => {
                parts.pop()?;
            }
            Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    let normalized = parts.into_iter().collect::<PathBuf>();
    (!normalized.as_os_str().is_empty()).then_some(normalized)
}
