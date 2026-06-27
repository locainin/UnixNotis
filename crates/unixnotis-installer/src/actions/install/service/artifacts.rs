//! Service artifact write and removal coordinator

use std::sync::atomic::Ordering;

use anyhow::{anyhow, Context, Result};

use crate::service_manager::{ServiceArtifact, ServiceArtifactKind};

use super::super::super::ActionContext;
use super::{dirs, files, symlinks};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::actions::install) enum ServiceArtifactWrite {
    CreatedOrUpdated,
    Unchanged,
}

pub(in crate::actions::install) fn write_service_artifacts(
    ctx: &mut ActionContext,
) -> Result<ServiceArtifactWrite> {
    let steady_artifacts = ctx.paths.service.artifacts(&ctx.paths.bin_dir);
    let install_artifacts = ctx.paths.service.install_artifacts(&ctx.paths.bin_dir);
    let mut steady_changed = false;
    for artifact in &install_artifacts {
        // Install artifacts can include short-lived ordering gates, such as runit's down file
        // Only steady artifacts should make later reload/start logging claim service bytes changed
        let changed = write_service_artifact(ctx, artifact)?;
        if install_artifact_is_steady(&steady_artifacts, artifact) {
            steady_changed |= changed;
        }
    }
    for artifact in ctx.paths.service.pre_start_artifacts_to_write() {
        // Start gates are temporary and should not affect steady install-state checks
        // Runit uses this to prevent supervisor races while envdir files are still being written
        write_service_artifact(ctx, &artifact)?;
    }

    // Reload only matters when the active service manager has new bytes to pick up
    ctx.service_reload_required
        .store(steady_changed, Ordering::Release);
    if steady_changed {
        Ok(ServiceArtifactWrite::CreatedOrUpdated)
    } else {
        Ok(ServiceArtifactWrite::Unchanged)
    }
}

fn install_artifact_is_steady(
    steady_artifacts: &[ServiceArtifact],
    artifact: &ServiceArtifact,
) -> bool {
    // Equality includes the expected path, shape, contents, and mode
    // That keeps temporary install-only files from masquerading as service changes
    steady_artifacts.iter().any(|steady| steady == artifact)
}

pub(crate) fn write_service_artifact(
    ctx: &ActionContext,
    artifact: &ServiceArtifact,
) -> Result<bool> {
    if let Some(parent) = artifact.path.parent() {
        // Parent setup walks one component at a time so symlinks cannot redirect writes
        dirs::ensure_directory_without_symlink(parent)
            .with_context(|| format!("failed to prepare {}", ctx.paths.service.artifact_label()))?;
    }

    match &artifact.kind {
        ServiceArtifactKind::File | ServiceArtifactKind::ExecutableFile => {
            // File-like artifacts require bytes; directories and links intentionally do not
            let contents = artifact
                .contents
                .as_ref()
                .ok_or_else(|| anyhow!("service file artifact missing contents"))?;
            files::write_regular_service_file(
                &artifact.path,
                contents,
                artifact.mode,
                ctx.paths.service.artifact_label(),
            )
        }
        // Directory ownership rules live in dirs.rs so this coordinator stays readable
        ServiceArtifactKind::Directory => dirs::write_directory_artifact(&artifact.path),
        ServiceArtifactKind::ManagedDirectory => dirs::write_managed_directory(&artifact.path),
        // Symlink handling checks the link itself and never follows it
        ServiceArtifactKind::Symlink { target } => {
            symlinks::write_service_symlink(&artifact.path, target)
        }
    }
}

pub(in crate::actions::install) fn remove_service_artifact(
    artifact: &ServiceArtifact,
) -> Result<()> {
    // Removal is shape-aware so uninstall never follows service-manager symlinks
    match &artifact.kind {
        ServiceArtifactKind::Directory => {
            // Plain directories are removed only when empty
            if dirs::service_artifact_path_is_present(&artifact.path) {
                dirs::remove_empty_service_directory(&artifact.path)?;
            }
        }
        ServiceArtifactKind::ManagedDirectory => {
            // Managed directories can be recursive, so dirs.rs verifies the ownership marker
            if dirs::service_artifact_path_is_present(&artifact.path) {
                dirs::remove_managed_directory(&artifact.path)?;
            }
        }
        ServiceArtifactKind::File | ServiceArtifactKind::ExecutableFile => {
            // File removal rejects symlinks even when the install state saw a path earlier
            if dirs::service_artifact_path_is_present(&artifact.path) {
                files::remove_regular_service_file(&artifact.path)?;
            }
        }
        ServiceArtifactKind::Symlink { target } => {
            // Link removal requires the expected target to still match
            symlinks::remove_service_symlink(&artifact.path, target)?;
        }
    }
    Ok(())
}

pub(in crate::actions::install) fn service_artifact_path_exists(
    artifact: &ServiceArtifact,
) -> bool {
    // Callers use the same shape checks as install and uninstall safety gates
    artifact.is_present_safely()
}

pub(in crate::actions::install) fn service_artifact_path_conflicts(
    artifact: &ServiceArtifact,
) -> bool {
    // Conflicts are real paths that fail the safe ownership/shape check
    artifact.exists_at_path_but_not_safely()
}
