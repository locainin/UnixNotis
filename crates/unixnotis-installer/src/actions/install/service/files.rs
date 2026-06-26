//! Regular service artifact file writes and removals

use std::fs;
use std::io::ErrorKind;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use anyhow::{anyhow, Context, Result};

use crate::paths::format_with_home;

use super::super::super::config::backup::write_atomic;

pub(in crate::actions::install::service) fn write_regular_service_file(
    path: &Path,
    contents: &str,
    mode: Option<u32>,
    artifact_label: &str,
) -> Result<bool> {
    // Refuse unsafe existing paths before looking at file contents
    let existed_before = ensure_regular_artifact_file_path(path)?;
    let changed = match fs::read_to_string(path) {
        // Stable contents keep reinstall quiet and avoid unnecessary manager reloads
        Ok(existing) if existing == contents => false,
        Ok(_) | Err(_) => {
            // Atomic writes avoid half-written service definitions on interruption
            write_atomic(path, contents)
                .with_context(|| format!("failed to write {artifact_label}"))?;
            true
        }
    };

    if let Some(mode) = mode {
        // Only artifacts that requested a mode receive chmod
        #[cfg(unix)]
        {
            // Mode is explicit because service scripts must not depend on process umask
            fs::set_permissions(path, fs::Permissions::from_mode(mode))
                .with_context(|| format!("failed to chmod {}", format_with_home(path)))?;
        }
        #[cfg(not(unix))]
        {
            return Err(anyhow!(
                "cannot apply executable mode {} on non-Unix platforms",
                mode
            ));
        }
    }

    Ok(changed || !existed_before)
}

pub(in crate::actions::install::service) fn ensure_regular_artifact_file_path(
    path: &Path,
) -> Result<bool> {
    // Existing service files may be replaced only when the old path is a plain file
    match fs::symlink_metadata(path) {
        // Replacing a symlink would write through attacker-controlled filesystem state
        Ok(metadata) if metadata.file_type().is_symlink() => Err(anyhow!(
            "cannot replace symlink service artifact at {}",
            format_with_home(path)
        )),
        // File artifacts cannot take over directories owned by another backend layout
        Ok(metadata) if metadata.is_dir() => Err(anyhow!(
            "cannot replace directory service artifact at {}",
            format_with_home(path)
        )),
        // Regular files are safe to compare and replace through the atomic writer
        Ok(metadata) if metadata.file_type().is_file() => Ok(true),
        // Sockets, fifos, and device nodes can block or behave strangely when read
        Ok(_) => Err(anyhow!(
            "cannot replace non-regular service artifact at {}",
            format_with_home(path)
        )),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(false),
        Err(err) => {
            Err(err).with_context(|| format!("failed to inspect {}", format_with_home(path)))
        }
    }
}

pub(in crate::actions::install::service) fn remove_regular_service_file(path: &Path) -> Result<()> {
    // File removal checks the final path again so links are not followed on uninstall
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("failed to inspect {}", format_with_home(path)))?;
    if metadata.file_type().is_symlink() {
        // Removing link artifacts goes through the symlink-specific path with target checks
        return Err(anyhow!(
            "refusing to remove symlink service file at {}",
            format_with_home(path)
        ));
    }
    if !metadata.file_type().is_file() {
        // Directories are handled separately because recursive removal needs an ownership marker
        return Err(anyhow!(
            "refusing to remove non-file service artifact at {}",
            format_with_home(path)
        ));
    }

    fs::remove_file(path).with_context(|| format!("failed to remove {}", format_with_home(path)))
}
