//! File walking and display-path helpers for css-check

use anyhow::{Context, Result};
use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

pub(super) fn collect_css_files(root: &Path) -> Result<Vec<PathBuf>> {
    // Canonical root keeps symlink checks stable
    let canonical_root = fs::canonicalize(root)
        .with_context(|| format!("resolve config directory {}", root.display()))?;
    let mut visited: HashSet<PathBuf> = HashSet::new();
    visited.insert(canonical_root.clone());

    // Depth-first walk is enough here and keeps memory small
    let mut stack = vec![root.to_path_buf()];
    let mut results = Vec::new();
    while let Some(dir) = stack.pop() {
        let entries = fs::read_dir(&dir)
            .with_context(|| format!("read config directory {}", dir.display()))?;
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            let file_type = entry.file_type()?;
            let is_dir = if file_type.is_dir() {
                true
            } else if file_type.is_symlink() {
                // Shared theme trees may use symlinked folders
                path.is_dir()
            } else {
                false
            };

            if is_dir {
                if is_backup_dir(&path) {
                    continue;
                }
                if let Ok(canonical) = fs::canonicalize(&path) {
                    // Stay inside the config tree even when symlinks point elsewhere
                    if !canonical.starts_with(&canonical_root) {
                        continue;
                    }
                    if !visited.insert(canonical) {
                        continue;
                    }
                }
                stack.push(path);
                continue;
            }

            if is_css_file(&path) {
                results.push(path);
            }
        }
    }
    results.sort();
    Ok(results)
}

fn is_backup_dir(path: &Path) -> bool {
    // Backup folders use the Backup-YYYY-MM-DD name shape
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.starts_with("Backup-"))
        .unwrap_or(false)
}

fn is_css_file(path: &Path) -> bool {
    // Only *.css files matter here
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("css"))
        .unwrap_or(false)
}

pub(super) fn display_config_root(config_dir: &Path) -> String {
    // Show env-rooted paths when possible so output is stable across machines
    if let Ok(xdg) = env::var("XDG_CONFIG_HOME") {
        let trimmed = xdg.trim();
        if !trimmed.is_empty() {
            let path = PathBuf::from(trimmed);
            if path.is_absolute() && config_dir == path.join("unixnotis") {
                return "$XDG_CONFIG_HOME/unixnotis".to_string();
            }
        }
    }

    if let Ok(home) = env::var("HOME") {
        let path = PathBuf::from(home).join(".config").join("unixnotis");
        if config_dir == path {
            return "$HOME/.config/unixnotis".to_string();
        }
    }

    config_dir.display().to_string()
}

pub(super) fn format_display_path(config_dir: &Path, display_root: &str, path: &Path) -> String {
    // Trim the absolute config prefix when the file lives under it
    if let Ok(relative) = path.strip_prefix(config_dir) {
        if relative.as_os_str().is_empty() {
            return display_root.to_string();
        }
        return format!("{}/{}", display_root, relative.display());
    }

    path.display().to_string()
}
