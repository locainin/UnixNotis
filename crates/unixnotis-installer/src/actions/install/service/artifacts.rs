//! Filesystem writes and removals for service-manager artifacts

use std::fs;
use std::io::ErrorKind;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::Ordering;

use anyhow::{anyhow, Context, Result};

use crate::paths::format_with_home;
use crate::service_manager::{ServiceArtifact, ServiceArtifactKind, MANAGED_DIRECTORY_MARKER};

use super::super::super::{config::backup::write_atomic, ActionContext};

const MANAGED_DIRECTORY_MARKER_CONTENTS: &str = "unixnotis\n";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::actions::install) enum ServiceArtifactWrite {
    CreatedOrUpdated,
    Unchanged,
}

pub(in crate::actions::install) fn write_service_artifacts(
    ctx: &mut ActionContext,
) -> Result<ServiceArtifactWrite> {
    let artifacts = ctx.paths.service.artifacts(&ctx.paths.bin_dir);
    let mut changed = false;
    for artifact in &artifacts {
        // Each artifact decides its own filesystem shape so backends are not forced into unit files
        changed |= write_service_artifact(ctx, artifact)?;
    }
    for artifact in ctx.paths.service.pre_start_artifacts_to_write() {
        // Start gates are temporary and should not affect steady install-state checks
        write_service_artifact(ctx, &artifact)?;
    }

    // Reload only matters when the active service manager has new bytes to pick up
    ctx.service_reload_required
        .store(changed, Ordering::Release);
    if changed {
        Ok(ServiceArtifactWrite::CreatedOrUpdated)
    } else {
        Ok(ServiceArtifactWrite::Unchanged)
    }
}

pub(crate) fn write_service_artifact(
    ctx: &ActionContext,
    artifact: &ServiceArtifact,
) -> Result<bool> {
    if let Some(parent) = artifact.path.parent() {
        // Parent setup walks one component at a time so symlinks cannot redirect writes
        ensure_directory_without_symlink(parent)
            .with_context(|| format!("failed to prepare {}", ctx.paths.service.artifact_label()))?;
    }

    match &artifact.kind {
        ServiceArtifactKind::File | ServiceArtifactKind::ExecutableFile => {
            let contents = artifact
                .contents
                .as_ref()
                .ok_or_else(|| anyhow!("service file artifact missing contents"))?;
            let existed_before = ensure_regular_artifact_file_path(&artifact.path)?;
            let changed = match fs::read_to_string(&artifact.path) {
                // Stable contents keep reinstall quiet and avoid unnecessary manager reloads
                Ok(existing) if existing == *contents => false,
                Ok(_) | Err(_) => {
                    write_atomic(&artifact.path, contents).with_context(|| {
                        format!("failed to write {}", ctx.paths.service.artifact_label())
                    })?;
                    true
                }
            };
            if let Some(mode) = artifact.mode {
                #[cfg(unix)]
                {
                    // Mode is explicit because service scripts must not depend on process umask
                    fs::set_permissions(&artifact.path, fs::Permissions::from_mode(mode))
                        .with_context(|| {
                            format!("failed to chmod {}", format_with_home(&artifact.path))
                        })?;
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
        ServiceArtifactKind::Directory => {
            let existed_before = ensure_artifact_directory_path(&artifact.path)?;
            ensure_directory_without_symlink(&artifact.path).with_context(|| {
                format!("failed to create {}", format_with_home(&artifact.path))
            })?;
            Ok(!existed_before)
        }
        ServiceArtifactKind::ManagedDirectory => write_managed_directory(&artifact.path),
        ServiceArtifactKind::Symlink { target } => write_service_symlink(&artifact.path, target),
    }
}

fn ensure_directory_without_symlink(path: &Path) -> Result<()> {
    let mut current = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => current.push(prefix.as_os_str()),
            Component::RootDir => current.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                return Err(anyhow!(
                    "refusing parent traversal in service artifact path {}",
                    format_with_home(path)
                ));
            }
            Component::Normal(part) => {
                current.push(part);
                match fs::symlink_metadata(&current) {
                    // symlink_metadata checks the path itself, not the linked target
                    Ok(metadata) if metadata.file_type().is_symlink() => {
                        return Err(anyhow!(
                            "refusing symlink parent component {}",
                            format_with_home(&current)
                        ));
                    }
                    Ok(metadata) if metadata.is_dir() => {}
                    Ok(_) => {
                        return Err(anyhow!(
                            "refusing non-directory parent component {}",
                            format_with_home(&current)
                        ));
                    }
                    Err(err) if err.kind() == ErrorKind::NotFound => {
                        fs::create_dir(&current).with_context(|| {
                            format!("failed to create {}", format_with_home(&current))
                        })?;
                    }
                    Err(err) => {
                        return Err(err).with_context(|| {
                            format!("failed to inspect {}", format_with_home(&current))
                        });
                    }
                }
            }
        }
    }
    Ok(())
}

fn ensure_regular_artifact_file_path(path: &Path) -> Result<bool> {
    // Existing service files may be replaced, but never through a symlink or directory
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(anyhow!(
            "cannot replace symlink service artifact at {}",
            format_with_home(path)
        )),
        Ok(metadata) if metadata.is_dir() => Err(anyhow!(
            "cannot replace directory service artifact at {}",
            format_with_home(path)
        )),
        Ok(_) => Ok(true),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(false),
        Err(err) => {
            Err(err).with_context(|| format!("failed to inspect {}", format_with_home(path)))
        }
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

fn write_managed_directory(path: &Path) -> Result<bool> {
    let existed_before = ensure_artifact_directory_path(path)?;
    ensure_directory_without_symlink(path)
        .with_context(|| format!("failed to create {}", format_with_home(path)))?;

    let marker = managed_directory_marker(path);
    if existed_before && !managed_marker_is_valid(&marker) {
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
            write_atomic(&marker, MANAGED_DIRECTORY_MARKER_CONTENTS)
                .with_context(|| format!("failed to write {}", format_with_home(&marker)))?;
            true
        }
    };
    Ok(!existed_before || marker_changed)
}

fn write_service_symlink(path: &Path, target: &Path) -> Result<bool> {
    if let Ok(existing) = fs::read_link(path) {
        if existing == target {
            // Relative links are compared as stored, matching how the backend declared them
            return Ok(false);
        }
        fs::remove_file(path)
            .with_context(|| format!("failed to replace {}", format_with_home(path)))?;
    } else {
        match fs::symlink_metadata(path) {
            Ok(_) => {
                return Err(anyhow!(
                    "cannot replace non-symlink service artifact at {}",
                    format_with_home(path)
                ));
            }
            Err(err) if err.kind() == ErrorKind::NotFound => {}
            Err(err) => {
                return Err(err)
                    .with_context(|| format!("failed to inspect {}", format_with_home(path)));
            }
        }
    }
    std::os::unix::fs::symlink(target, path)
        .with_context(|| format!("failed to create symlink {}", format_with_home(path)))?;
    Ok(true)
}

pub(in crate::actions::install) fn remove_service_artifact(
    artifact: &ServiceArtifact,
) -> Result<()> {
    // Removal is shape-aware so uninstall never follows service-manager symlinks
    match &artifact.kind {
        ServiceArtifactKind::Directory => {
            if service_artifact_path_is_present(&artifact.path) {
                remove_empty_service_directory(&artifact.path)?;
            }
        }
        ServiceArtifactKind::ManagedDirectory => {
            if service_artifact_path_is_present(&artifact.path) {
                remove_managed_directory(&artifact.path)?;
            }
        }
        ServiceArtifactKind::File | ServiceArtifactKind::ExecutableFile => {
            if service_artifact_path_is_present(&artifact.path) {
                remove_regular_service_file(&artifact.path)?;
            }
        }
        ServiceArtifactKind::Symlink { target } => remove_service_symlink(&artifact.path, target)?,
    }
    Ok(())
}

fn service_artifact_path_is_present(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok()
}

fn remove_empty_service_directory(path: &Path) -> Result<()> {
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

fn remove_managed_directory(path: &Path) -> Result<()> {
    let marker = managed_directory_marker(path);
    // Managed directories can contain backend files, so the marker gates recursive removal
    if !managed_marker_is_valid(&marker) {
        return Err(anyhow!(
            "refusing to recursively remove unmarked service directory at {}",
            format_with_home(path)
        ));
    }
    fs::remove_dir_all(path).with_context(|| format!("failed to remove {}", format_with_home(path)))
}

fn managed_marker_is_valid(path: &Path) -> bool {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return false;
    };
    // A marker symlink is not ownership proof because it can point outside the service dir
    if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
        return false;
    }
    fs::read_to_string(path)
        .map(|contents| contents == MANAGED_DIRECTORY_MARKER_CONTENTS)
        .unwrap_or(false)
}

fn managed_directory_marker(path: &Path) -> PathBuf {
    path.join(MANAGED_DIRECTORY_MARKER)
}

fn remove_regular_service_file(path: &Path) -> Result<()> {
    // File removal checks the final path again so links are not followed on uninstall
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("failed to inspect {}", format_with_home(path)))?;
    if metadata.file_type().is_symlink() {
        return Err(anyhow!(
            "refusing to remove symlink service file at {}",
            format_with_home(path)
        ));
    }
    if !metadata.file_type().is_file() {
        return Err(anyhow!(
            "refusing to remove non-file service artifact at {}",
            format_with_home(path)
        ));
    }
    fs::remove_file(path).with_context(|| format!("failed to remove {}", format_with_home(path)))
}

fn remove_service_symlink(path: &Path, expected_target: &Path) -> Result<()> {
    // Symlink artifacts are removed only when both the type and target still match
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(()),
        Err(err) => {
            return Err(err)
                .with_context(|| format!("failed to inspect {}", format_with_home(path)));
        }
    };
    if !metadata.file_type().is_symlink() {
        return Err(anyhow!(
            "refusing to remove non-symlink service artifact at {}",
            format_with_home(path)
        ));
    }

    let actual_target = fs::read_link(path)
        .with_context(|| format!("failed to read symlink {}", format_with_home(path)))?;
    if actual_target != expected_target {
        return Err(anyhow!(
            "refusing to remove symlink {} because it points to {} instead of {}",
            format_with_home(path),
            format_with_home(&actual_target),
            format_with_home(expected_target)
        ));
    }

    fs::remove_file(path).with_context(|| format!("failed to remove {}", format_with_home(path)))
}

pub(in crate::actions::install) fn service_artifact_path_exists(
    artifact: &ServiceArtifact,
) -> bool {
    // symlink_metadata observes the artifact path itself instead of following service links
    match &artifact.kind {
        ServiceArtifactKind::File | ServiceArtifactKind::ExecutableFile => {
            fs::symlink_metadata(&artifact.path)
                .map(|metadata| metadata.file_type().is_file())
                .unwrap_or(false)
        }
        ServiceArtifactKind::Directory => fs::symlink_metadata(&artifact.path)
            .map(|metadata| metadata.file_type().is_dir())
            .unwrap_or(false),
        ServiceArtifactKind::ManagedDirectory => {
            fs::symlink_metadata(&artifact.path)
                .map(|metadata| metadata.file_type().is_dir())
                .unwrap_or(false)
                && managed_marker_is_valid(&managed_directory_marker(&artifact.path))
        }
        ServiceArtifactKind::Symlink { target } => fs::read_link(&artifact.path)
            .map(|actual| actual == *target)
            .unwrap_or(false),
    }
}
