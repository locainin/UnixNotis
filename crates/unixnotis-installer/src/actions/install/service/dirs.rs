//! Service artifact directory creation and guarded directory removal

use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use unixnotis_core::filesystem::{
    create_directory_all, remove_directory_tree, remove_empty_directory, write_file_atomic,
};

use crate::paths::format_with_home;
use crate::service_manager::{
    managed_directory_marker, managed_directory_marker_is_valid, MANAGED_DIRECTORY_MARKER_CONTENTS,
};

use super::files::ensure_regular_artifact_file_path;

pub(in crate::actions::install::service) fn write_directory_artifact(path: &Path) -> Result<bool> {
    // Plain directories are container nodes only, so they must already be real directories
    let existed_before = ensure_artifact_directory_path(path)?;
    // Parent and final directory creation share the same no-symlink walk
    ensure_directory_without_symlink(path)
        .with_context(|| format!("failed to create {}", format_with_home(path)))?;
    Ok(!existed_before)
}

pub(in crate::actions::install::service) fn write_managed_directory(path: &Path) -> Result<bool> {
    // Managed directories are the only artifact type allowed to contain nested backend files
    let existed_before = ensure_artifact_directory_path(path)?;
    // Create the directory before marker validation so first install can seed ownership
    ensure_directory_without_symlink(path)
        .with_context(|| format!("failed to create {}", format_with_home(path)))?;

    let marker = managed_directory_marker(path);
    if existed_before && !managed_directory_marker_is_valid(&marker) {
        // Existing service directories need proof of ownership before UnixNotis manages them
        return Err(anyhow!(
            "refusing to manage unmarked service directory at {}",
            format_with_home(path)
        ));
    }

    ensure_regular_artifact_file_path(&marker)?;
    let marker_changed = match fs::read_to_string(&marker) {
        // Marker contents stay tiny and exact so foreign files are not treated as ownership
        Ok(existing) if existing == MANAGED_DIRECTORY_MARKER_CONTENTS => false,
        Ok(_) | Err(_) => {
            // The marker itself is written atomically so partial writes do not grant ownership
            write_file_atomic(&marker, MANAGED_DIRECTORY_MARKER_CONTENTS.as_bytes(), 0o644)
                .with_context(|| format!("failed to write {}", format_with_home(&marker)))?;
            true
        }
    };

    Ok(!existed_before || marker_changed)
}

pub(in crate::actions::install::service) fn ensure_directory_without_symlink(
    path: &Path,
) -> Result<()> {
    // Core keeps one descriptor per component so parent swaps cannot redirect creation
    create_directory_all(path, 0o755)
        .map(|_created| ())
        .map_err(|error| {
            anyhow!(
                "refusing unsafe service directory path {}: {}",
                format_with_home(path),
                error
            )
        })
}

pub(in crate::actions::install::service) fn service_artifact_path_is_present(path: &Path) -> bool {
    // symlink_metadata observes the artifact path itself instead of following service links
    fs::symlink_metadata(path).is_ok()
}

pub(in crate::actions::install::service) fn remove_empty_service_directory(
    path: &Path,
) -> Result<()> {
    // Plain directory artifacts are removed only when empty to preserve shared parent state
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("failed to inspect {}", format_with_home(path)))?;
    if metadata.file_type().is_symlink() {
        return Err(anyhow!(
            "refusing to remove symlink service directory at {}",
            format_with_home(path)
        ));
    }
    if !metadata.file_type().is_dir() {
        return Err(anyhow!(
            "refusing to remove non-directory service artifact at {}",
            format_with_home(path)
        ));
    }

    if remove_empty_directory(path)
        .with_context(|| format!("failed to remove {}", format_with_home(path)))?
    {
        Ok(())
    } else {
        Err(anyhow!(
            "service directory disappeared before removal at {}",
            format_with_home(path)
        ))
    }
}

pub(in crate::actions::install::service) fn remove_managed_directory(path: &Path) -> Result<()> {
    let marker = managed_directory_marker(path);
    // Managed directories can contain backend files, so the marker gates recursive removal
    if !managed_directory_marker_is_valid(&marker) {
        return Err(anyhow!(
            "refusing to recursively remove unmarked service directory at {}",
            format_with_home(path)
        ));
    }

    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("failed to inspect {}", format_with_home(path)))?;
    // Recheck the root immediately before deletion so a swapped symlink is not removed
    if metadata.file_type().is_symlink() || !metadata.file_type().is_dir() {
        return Err(anyhow!(
            "refusing to recursively remove unsafe service directory at {}",
            format_with_home(path)
        ));
    }

    if remove_directory_tree(path)
        .with_context(|| format!("failed to remove {}", format_with_home(path)))?
    {
        Ok(())
    } else {
        Err(anyhow!(
            "managed service directory disappeared before removal at {}",
            format_with_home(path)
        ))
    }
}

fn ensure_artifact_directory_path(path: &Path) -> Result<bool> {
    // Directory artifacts are container paths, so replacing files or links would be surprising
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(anyhow!(
            "cannot replace symlink service directory at {}",
            format_with_home(path)
        )),
        Ok(metadata) if !metadata.is_dir() => Err(anyhow!(
            "cannot replace non-directory service artifact at {}",
            format_with_home(path)
        )),
        Ok(_) => Ok(true),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(false),
        Err(err) => {
            Err(err).with_context(|| format!("failed to inspect {}", format_with_home(path)))
        }
    }
}
