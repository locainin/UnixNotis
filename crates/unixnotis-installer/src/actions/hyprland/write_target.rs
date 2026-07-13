//! Secure write-path resolution for Hyprland dotfile symlinks

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};

use crate::paths::{format_with_home, home_dir};

pub(super) fn resolve_hyprland_write_path(path: &Path) -> Result<PathBuf> {
    let metadata = fs::symlink_metadata(path).with_context(|| {
        format!(
            "failed to inspect Hyprland config {}",
            format_with_home(path)
        )
    })?;
    if metadata.is_file() {
        // Ordinary files keep their display path while the secure writer checks every ancestor
        return Ok(path.to_path_buf());
    }
    if !metadata.file_type().is_symlink() {
        return Err(anyhow!(
            "refusing non-file Hyprland config {}",
            format_with_home(path)
        ));
    }

    // Canonicalization resolves the complete link chain before any bytes are read or replaced
    let resolved = fs::canonicalize(path).with_context(|| {
        format!(
            "failed to resolve Hyprland config symlink {}",
            format_with_home(path)
        )
    })?;
    let resolved_metadata = fs::metadata(&resolved).with_context(|| {
        format!(
            "failed to inspect resolved Hyprland config {}",
            format_with_home(&resolved)
        )
    })?;
    if !resolved_metadata.is_file() {
        return Err(anyhow!(
            "refusing Hyprland config symlink to non-file {}",
            format_with_home(&resolved)
        ));
    }

    // Dotfile links may leave XDG_CONFIG_HOME but must stay inside the current home directory
    let canonical_home = fs::canonicalize(home_dir()?)
        .with_context(|| "failed to resolve home directory for Hyprland config safety check")?;
    if !resolved.starts_with(&canonical_home) {
        return Err(anyhow!(
            "refusing Hyprland config symlink outside the home directory: {}",
            format_with_home(&resolved)
        ));
    }

    // The canonical target has no symlink components, so the atomic writer can use NO_SYMLINKS
    Ok(resolved)
}
