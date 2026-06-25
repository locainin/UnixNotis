//! Service artifact install and lifecycle helpers

use std::fs;
use std::io::ErrorKind;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::atomic::Ordering;

use anyhow::{anyhow, Context, Result};

use crate::paths::format_with_home;
use crate::service_manager::{CommandSpec, ServiceArtifact, ServiceArtifactKind};

use super::super::{
    config::backup::write_atomic,
    ensure_shell_path_entry,
    hyprland::{ensure_hyprland_autostart, remove_hyprland_autostart},
    log_line, run_command, sync_user_environment, ActionContext,
};

pub(crate) fn install_service(ctx: &mut ActionContext) -> Result<()> {
    match write_service_artifacts(ctx)? {
        ServiceArtifactWrite::CreatedOrUpdated => {
            log_line(
                ctx,
                format!(
                    "Installed {} at {}",
                    ctx.paths.service.artifact_label(),
                    format_with_home(&ctx.paths.service.primary_artifact_path())
                ),
            );
        }
        ServiceArtifactWrite::Unchanged => {
            log_line(
                ctx,
                format!("{} already up to date", ctx.paths.service.artifact_label()),
            );
        }
    }

    Ok(())
}

pub(crate) fn enable_service(ctx: &mut ActionContext) -> Result<()> {
    if ctx.service_reload_required.load(Ordering::Acquire) {
        // A full user-manager reload is expensive on some setups, so run it only when needed
        log_line(
            ctx,
            format!("Reloading {}", ctx.paths.service.manager_label()),
        );
        if let Some(spec) = ctx.paths.service.reload_after_artifact_change() {
            run_command_spec(ctx, &spec)?;
        }
    } else {
        log_line(
            ctx,
            format!(
                "Skipping {} reload because {} is unchanged",
                ctx.paths.service.manager_label(),
                ctx.paths.service.artifact_label()
            ),
        );
    }

    // Import the live session env first so the first daemon start picks it up
    sync_user_environment(ctx)?;
    remove_pre_start_artifacts(ctx)?;
    run_service_start(ctx)?;

    // Shell startup files are updated so new terminals can resolve the installed commands
    if let Err(err) = ensure_shell_path_entry(ctx) {
        log_line(
            ctx,
            format!("Warning: failed to update shell PATH files ({err})"),
        );
    }

    // Hyprland gets one managed exec-once block so session env sync happens once per login
    ensure_hyprland_autostart(ctx);
    Ok(())
}

pub(crate) fn uninstall_service(ctx: &mut ActionContext) -> Result<()> {
    let artifacts = ctx.paths.service.artifacts(&ctx.paths.bin_dir);
    let artifact_exists = artifacts.iter().any(service_artifact_path_exists);

    if artifact_exists {
        if let Some(spec) = ctx.paths.service.disable_now_command() {
            if let Err(err) = run_command_spec(ctx, &spec) {
                log_line(ctx, format!("Warning: {}", err));
            }
        } else {
            log_line(
                ctx,
                format!(
                    "Skipping disable; {} has no disable command",
                    ctx.paths.service.label()
                ),
            );
        }

        for artifact in artifacts.iter().rev() {
            remove_service_artifact(artifact).with_context(|| {
                format!(
                    "failed to remove {} at {}",
                    ctx.paths.service.artifact_label(),
                    format_with_home(&artifact.path)
                )
            })?;
            log_line(
                ctx,
                format!(
                    "Removed {} at {}",
                    ctx.paths.service.artifact_label(),
                    format_with_home(&artifact.path)
                ),
            );
        }
        if let Some(spec) = ctx.paths.service.reload_after_artifact_change() {
            run_command_spec(ctx, &spec)?;
        }
    } else {
        log_line(
            ctx,
            format!(
                "{} not found at {}",
                ctx.paths.service.artifact_label(),
                format_with_home(&ctx.paths.service.primary_artifact_path())
            ),
        );
    }

    remove_hyprland_autostart(ctx);
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::actions::install) enum ServiceArtifactWrite {
    CreatedOrUpdated,
    Unchanged,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::actions::install) enum ServiceStartMode {
    EnableAndStart,
    StartOnly,
}

fn write_service_artifacts(ctx: &mut ActionContext) -> Result<ServiceArtifactWrite> {
    let artifacts = ctx.paths.service.artifacts(&ctx.paths.bin_dir);
    let mut changed = false;
    for artifact in &artifacts {
        // Each artifact decides its own filesystem shape so backends are not forced into unit files
        changed |= write_service_artifact(ctx, artifact)?;
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
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create {} directory",
                ctx.paths.service.artifact_label()
            )
        })?;
    }

    match &artifact.kind {
        ServiceArtifactKind::File | ServiceArtifactKind::ExecutableFile => {
            let contents = artifact
                .contents
                .as_ref()
                .ok_or_else(|| anyhow!("service file artifact missing contents"))?;
            let existed_before = ensure_regular_artifact_file_path(&artifact.path)?;
            let changed = match fs::read_to_string(&artifact.path) {
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
            fs::create_dir_all(&artifact.path).with_context(|| {
                format!("failed to create {}", format_with_home(&artifact.path))
            })?;
            Ok(!existed_before)
        }
        ServiceArtifactKind::Symlink { target } => write_service_symlink(&artifact.path, target),
    }
}

fn ensure_regular_artifact_file_path(path: &Path) -> Result<bool> {
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

fn write_service_symlink(path: &Path, target: &Path) -> Result<bool> {
    if let Ok(existing) = fs::read_link(path) {
        if existing == target {
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
    match &artifact.kind {
        ServiceArtifactKind::Directory => {
            if service_artifact_path_exists(artifact) {
                // Only the backend-owned service directory is removed
                // Parent supervision roots are never recursively cleaned up here
                fs::remove_dir_all(&artifact.path)?;
            }
        }
        ServiceArtifactKind::File | ServiceArtifactKind::ExecutableFile => {
            if service_artifact_path_exists(artifact) {
                remove_regular_service_file(&artifact.path)?;
            }
        }
        ServiceArtifactKind::Symlink { target } => remove_service_symlink(&artifact.path, target)?,
    }
    Ok(())
}

fn remove_regular_service_file(path: &Path) -> Result<()> {
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

fn service_artifact_path_exists(artifact: &ServiceArtifact) -> bool {
    // symlink_metadata observes the artifact path itself instead of following service links
    fs::symlink_metadata(&artifact.path).is_ok()
}

fn service_start_mode(ctx: &ActionContext) -> ServiceStartMode {
    // Cached install state keeps the reinstall branch stable for one installer run
    service_start_mode_from_enabled(
        ctx.install_state
            .as_ref()
            .map(|state| state.service_enabled()),
    )
}

pub(in crate::actions::install) fn service_start_mode_from_enabled(
    service_enabled: Option<bool>,
) -> ServiceStartMode {
    if service_enabled == Some(true) {
        // Reinstalls do not need another enable step, which can trigger a costly reload
        ServiceStartMode::StartOnly
    } else {
        ServiceStartMode::EnableAndStart
    }
}

fn run_service_start(ctx: &mut ActionContext) -> Result<()> {
    match service_start_mode(ctx) {
        ServiceStartMode::EnableAndStart => {
            // First install still needs the symlink creation done by `enable`
            log_line(
                ctx,
                format!("Enabling and starting {}", ctx.paths.service.service_name()),
            );
            let spec = ctx
                .paths
                .service
                .enable_now_command()
                .ok_or_else(|| anyhow!("service manager cannot enable and start service"))?;
            run_command_spec(ctx, &spec)
        }
        ServiceStartMode::StartOnly => {
            // Reinstall can start directly because the service is already enabled
            log_line(
                ctx,
                format!("Starting {}", ctx.paths.service.service_name()),
            );
            let spec = ctx
                .paths
                .service
                .start_command()
                .ok_or_else(|| anyhow!("service manager cannot start service"))?;
            run_command_spec(ctx, &spec)
        }
    }
}

fn remove_pre_start_artifacts(ctx: &mut ActionContext) -> Result<()> {
    let artifacts = ctx.paths.service.pre_start_artifacts_to_remove();
    for artifact in &artifacts {
        // Backend staging files are removed only after env sync has completed
        remove_service_artifact(artifact).with_context(|| {
            format!(
                "failed to remove start gate at {}",
                format_with_home(&artifact.path)
            )
        })?;
    }
    Ok(())
}

fn run_command_spec(ctx: &mut ActionContext, spec: &CommandSpec) -> Result<()> {
    run_command(ctx, spec.label(), spec.to_command(), None)
}
