//! Service artifact directory creation and guarded directory removal

use std::fs;
use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};

use anyhow::{anyhow, Context, Result};

use crate::paths::format_with_home;
use crate::service_manager::{
    managed_directory_marker, managed_directory_marker_is_valid, MANAGED_DIRECTORY_MARKER_CONTENTS,
};

use super::super::super::config::backup::write_atomic;
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
            write_atomic(&marker, MANAGED_DIRECTORY_MARKER_CONTENTS)
                .with_context(|| format!("failed to write {}", format_with_home(&marker)))?;
            true
        }
    };

    Ok(!existed_before || marker_changed)
}

pub(in crate::actions::install::service) fn ensure_directory_without_symlink(
    path: &Path,
) -> Result<()> {
    // Build the path one component at a time so an existing parent link cannot redirect writes
    let mut current = PathBuf::new();
    for component in path.components() {
        match component {
            // Windows prefixes are kept for correctness even though the installer is Unix-oriented
            Component::Prefix(prefix) => current.push(prefix.as_os_str()),
            Component::RootDir => current.push(component.as_os_str()),
            // Current-directory components do not change the resolved location
            Component::CurDir => {}
            Component::ParentDir => {
                // Parent traversal would make artifact ownership impossible to reason about
                return Err(anyhow!(
                    "refusing parent traversal in service artifact path {}",
                    format_with_home(path)
                ));
            }
            Component::Normal(part) => {
                current.push(part);
                inspect_or_create_directory_component(path, &current)?;
            }
        }
    }
    Ok(())
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

    fs::remove_dir(path).with_context(|| format!("failed to remove {}", format_with_home(path)))
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

    remove_managed_directory_tree(path)
        .with_context(|| format!("failed to remove {}", format_with_home(path)))
}

fn inspect_or_create_directory_component(full_path: &Path, current: &Path) -> Result<()> {
    // Every component is checked with symlink_metadata so the link itself is inspected
    match fs::symlink_metadata(current) {
        // symlink_metadata checks the path itself, not the linked target
        Ok(metadata) if metadata.file_type().is_symlink() => Err(anyhow!(
            "refusing symlink parent component {}",
            format_with_home(current)
        )),
        Ok(metadata) if metadata.is_dir() => Ok(()),
        Ok(_) => Err(anyhow!(
            "refusing non-directory parent component {}",
            format_with_home(current)
        )),
        // Missing components are created one at a time to avoid create_dir_all following links
        Err(err) if err.kind() == ErrorKind::NotFound => fs::create_dir(current)
            .with_context(|| format!("failed to create {}", format_with_home(current))),
        Err(err) => {
            Err(err).with_context(|| format!("failed to inspect {}", format_with_home(current)))
        }
    }
    .with_context(|| format!("while preparing {}", format_with_home(full_path)))
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

fn remove_managed_directory_tree(path: &Path) -> Result<()> {
    // Each level is inspected before reading children so symlink swaps do not get followed
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("failed to inspect {}", format_with_home(path)))?;
    if metadata.file_type().is_symlink() {
        return Err(anyhow!(
            "refusing symlink inside managed service directory at {}",
            format_with_home(path)
        ));
    }
    if !metadata.file_type().is_dir() {
        return Err(anyhow!(
            "refusing non-directory inside managed service directory at {}",
            format_with_home(path)
        ));
    }

    for entry in
        fs::read_dir(path).with_context(|| format!("failed to read {}", format_with_home(path)))?
    {
        let entry = entry.with_context(|| format!("failed to read {}", format_with_home(path)))?;
        let child = entry.path();
        let child_metadata = fs::symlink_metadata(&child)
            .with_context(|| format!("failed to inspect {}", format_with_home(&child)))?;

        if child_metadata.file_type().is_symlink() {
            // Backend-owned service directories should not need symlink children
            // Failing closed avoids deleting or traversing a path that changed under the installer
            return Err(anyhow!(
                "refusing symlink inside managed service directory at {}",
                format_with_home(&child)
            ));
        }
        if child_metadata.file_type().is_dir() {
            remove_managed_directory_tree(&child)?;
        } else if child_metadata.file_type().is_file() {
            fs::remove_file(&child)
                .with_context(|| format!("failed to remove {}", format_with_home(&child)))?;
        } else {
            // Sockets, fifos, and device nodes should not appear in installer-owned service trees
            return Err(anyhow!(
                "refusing special file inside managed service directory at {}",
                format_with_home(&child)
            ));
        }
    }

    fs::remove_dir(path).with_context(|| format!("failed to remove {}", format_with_home(path)))
}
