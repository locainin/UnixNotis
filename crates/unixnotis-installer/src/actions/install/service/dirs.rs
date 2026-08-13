//! Service artifact directory creation and guarded directory removal

use std::ffi::OsStr;
use std::fs;
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use unixnotis_core::filesystem::{
    create_directory_all, ensure_marked_directory, remove_empty_directory,
    remove_marked_directory_tree, CreateDirectoryOutcome,
};

use crate::paths::format_with_home;
use crate::service_manager::contract::{
    MANAGED_DIRECTORY_MARKER, MANAGED_DIRECTORY_MARKER_CONTENTS,
};

pub(in crate::actions::install::service) fn write_directory_artifact(path: &Path) -> Result<bool> {
    // The descriptor-backed result reflects the final component even after a create collision
    let outcome = create_directory_all(path, 0o755)
        .with_context(|| format!("failed to create {}", format_with_home(path)))?;
    Ok(outcome == CreateDirectoryOutcome::TargetCreated)
}

pub(in crate::actions::install::service) fn write_managed_directory(path: &Path) -> Result<bool> {
    let outcome = ensure_marked_directory(
        path,
        0o755,
        OsStr::new(MANAGED_DIRECTORY_MARKER),
        MANAGED_DIRECTORY_MARKER_CONTENTS.as_bytes(),
        0o644,
    )
    .map_err(|error| match error.kind() {
        std::io::ErrorKind::PermissionDenied => anyhow!(
            "refusing to manage unmarked service directory at {}: {}",
            format_with_home(path),
            error
        ),
        _ => anyhow!(
            "refusing unsafe service directory at {}: {}",
            format_with_home(path),
            error
        ),
    })?;
    Ok(outcome == CreateDirectoryOutcome::TargetCreated)
}

pub(in crate::actions::install::service) fn ensure_directory_without_symlink(
    path: &Path,
) -> Result<()> {
    // Core keeps one descriptor per component so parent swaps cannot redirect creation
    create_directory_all(path, 0o755)
        .map(|_outcome| ())
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
    let removed = remove_marked_directory_tree(
        path,
        OsStr::new(MANAGED_DIRECTORY_MARKER),
        MANAGED_DIRECTORY_MARKER_CONTENTS.as_bytes(),
    )
    .map_err(|error| match error.kind() {
        std::io::ErrorKind::PermissionDenied => anyhow!(
            "refusing to recursively remove unmarked service directory at {}: {}",
            format_with_home(path),
            error
        ),
        _ => anyhow!(
            "refusing to recursively remove unsafe service directory at {}: {}",
            format_with_home(path),
            error
        ),
    })?;
    if removed {
        Ok(())
    } else {
        Err(anyhow!(
            "managed service directory disappeared before removal at {}",
            format_with_home(path)
        ))
    }
}
