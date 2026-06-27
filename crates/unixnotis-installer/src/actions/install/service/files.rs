//! Regular service artifact file writes and removals

use std::fs;
use std::io::ErrorKind;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use anyhow::{anyhow, Context, Result};

use crate::paths::format_with_home;
use crate::service_manager::MANAGED_DIRECTORY_MARKER_CONTENTS;

use super::super::super::config::backup::write_atomic;

pub(in crate::actions::install::service) fn write_regular_service_file(
    path: &Path,
    contents: &str,
    mode: Option<u32>,
    artifact_label: &str,
) -> Result<bool> {
    // Refuse unsafe existing paths before looking at file contents
    let existed_before = ensure_regular_artifact_file_path(path)?;
    let mode_changed = match mode {
        Some(mode) => {
            #[cfg(unix)]
            {
                // Compare before chmod so reinstall stays quiet when both bytes and mode match
                current_mode(path)? != Some(mode)
            }
            #[cfg(not(unix))]
            {
                return Err(anyhow!(
                    "cannot apply executable mode {} on non-Unix platforms",
                    mode
                ));
            }
        }
        None => false,
    };
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
            if changed || mode_changed || !existed_before {
                // Mode is explicit because service scripts must not depend on process umask
                fs::set_permissions(path, fs::Permissions::from_mode(mode))
                    .with_context(|| format!("failed to chmod {}", format_with_home(path)))?;
            }
        }
    }

    Ok(changed || mode_changed || !existed_before)
}

pub(in crate::actions::install::service) fn write_shared_service_file(
    path: &Path,
    contents: &str,
    mode: Option<u32>,
    artifact_label: &str,
    created_marker: Option<&Path>,
) -> Result<bool> {
    // Shared files are setup anchors, not UnixNotis-owned replacement targets
    let existed_before = ensure_regular_artifact_file_path(path)?;
    if existed_before {
        let existing = fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", format_with_home(path)))?;
        if existing != contents {
            return Err(anyhow!(
                "refusing to overwrite shared service artifact at {}",
                format_with_home(path)
            ));
        }
        apply_artifact_mode_if_needed(path, mode)?;
        return Ok(false);
    }

    // Missing shared files can be seeded because no user contents are being replaced
    write_atomic(path, contents).with_context(|| format!("failed to write {artifact_label}"))?;
    apply_artifact_mode_if_needed(path, mode)?;
    if let Some(marker) = created_marker {
        write_shared_creation_marker(marker)?;
    }
    Ok(true)
}

pub(in crate::actions::install::service) fn remove_shared_service_file(
    path: &Path,
    created_marker: &Path,
) -> Result<bool> {
    if !shared_creation_marker_is_valid(created_marker) {
        // No marker means the shared file predated UnixNotis or has unknown ownership
        return Ok(false);
    }
    remove_regular_service_file(path)?;
    remove_regular_service_file(created_marker)?;
    remove_empty_shared_layout_dirs(path)?;
    Ok(true)
}

#[cfg(unix)]
fn current_mode(path: &Path) -> Result<Option<u32>> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => Ok(Some(metadata.permissions().mode() & 0o777)),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(None),
        Err(err) => {
            Err(err).with_context(|| format!("failed to inspect {}", format_with_home(path)))
        }
    }
}

fn apply_artifact_mode_if_needed(path: &Path, mode: Option<u32>) -> Result<()> {
    let Some(mode) = mode else {
        return Ok(());
    };

    #[cfg(unix)]
    {
        if current_mode(path)? != Some(mode) {
            // Shared support files still need explicit modes when the backend requests one
            fs::set_permissions(path, fs::Permissions::from_mode(mode))
                .with_context(|| format!("failed to chmod {}", format_with_home(path)))?;
        }
        Ok(())
    }

    #[cfg(not(unix))]
    {
        Err(anyhow!(
            "cannot apply executable mode {} on non-Unix platforms",
            mode
        ))
    }
}

fn write_shared_creation_marker(path: &Path) -> Result<()> {
    ensure_regular_artifact_file_path(path)?;
    write_atomic(path, MANAGED_DIRECTORY_MARKER_CONTENTS)
        .with_context(|| format!("failed to write {}", format_with_home(path)))
}

fn shared_creation_marker_is_valid(path: &Path) -> bool {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return false;
    };
    if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
        return false;
    }
    fs::read_to_string(path)
        .map(|contents| contents == MANAGED_DIRECTORY_MARKER_CONTENTS)
        .unwrap_or(false)
}

fn remove_empty_shared_layout_dirs(path: &Path) -> Result<()> {
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    // s6 default bundle initialization creates default/contents.d beside default/type
    // These directories are removed only when empty so user bundle members are preserved
    remove_dir_if_empty(&parent.join("contents.d"))?;
    remove_dir_if_empty(parent)
}

fn remove_dir_if_empty(path: &Path) -> Result<()> {
    match fs::remove_dir(path) {
        Ok(()) => Ok(()),
        Err(err)
            if matches!(
                err.kind(),
                ErrorKind::NotFound | ErrorKind::DirectoryNotEmpty
            ) =>
        {
            Ok(())
        }
        Err(err) => {
            Err(err).with_context(|| format!("failed to remove {}", format_with_home(path)))
        }
    }
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
