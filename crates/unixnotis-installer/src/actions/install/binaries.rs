//! Binary install and uninstall helpers

use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use unixnotis_core::filesystem::{
    read_symlink, remove_directory_tree, remove_regular_file, remove_symlink_if_target,
    RemoveSymlinkOutcome,
};

use crate::managed_binaries::validate_managed_binary_names;
use crate::paths::format_with_home;

use super::super::{
    binaries::{
        resolve_install_binaries, resolve_install_binaries_best_effort, resolve_target_directory,
    },
    log_line, ActionContext,
};
use crate::actions::daemon::DaemonActivationReservation;
use crate::actions::releases::install_release_generation_transaction;

pub fn install_binaries(
    ctx: &mut ActionContext,
    _reservation: &DaemonActivationReservation,
) -> Result<()> {
    let (binaries, release_dir) = resolve_install_inputs(ctx)?;
    let generation = install_release_generation_transaction(
        ctx.paths,
        &release_dir,
        &binaries,
        || Ok(()),
        || Ok(()),
        || crate::actions::daemon::ensure_selected_service_inactive(ctx.paths),
    )?;
    log_installed_generation(ctx, &binaries, &generation);
    Ok(())
}

pub(in crate::actions::install) fn resolve_install_inputs(
    ctx: &mut ActionContext,
) -> Result<(Vec<String>, PathBuf)> {
    // Read the managed binary list from installer metadata so install and uninstall stay aligned
    let binaries = resolve_install_binaries(ctx.paths)?;
    // Cargo metadata is the only reliable way to find the active release target directory
    let release_dir = resolve_release_dir(ctx)?;

    // Check every source before the versioned release transaction allocates staging state
    let mut missing = Vec::new();
    for binary in &binaries {
        let source = release_dir.join(binary);
        if !source.exists() {
            missing.push(format_with_home(&source));
        }
    }
    if !missing.is_empty() {
        return Err(anyhow!(
            "missing build artifacts (aborting before install): {}",
            missing.join(", ")
        ));
    }

    // Validate again at the copy boundary so future discovery changes cannot widen file access
    let binaries = validate_managed_binary_names(binaries)
        .with_context(|| "refusing to install an unmanaged binary path")?;
    Ok((binaries, release_dir))
}

pub(in crate::actions::install) fn log_installed_generation(
    ctx: &mut ActionContext,
    binaries: &[String],
    generation: &str,
) {
    log_line(
        ctx,
        format!("Activated complete UnixNotis release generation {generation}"),
    );
    for binary in binaries {
        log_line(
            ctx,
            format!(
                "Installed {binary} -> {}",
                format_with_home(&ctx.paths.bin_dir.join(binary))
            ),
        );
    }
}

pub fn remove_binaries(ctx: &mut ActionContext) -> Result<()> {
    // Best-effort discovery keeps uninstall usable even when the workspace is partially broken
    let (binaries, warning) = resolve_install_binaries_best_effort(ctx.paths);
    if let Some(message) = warning {
        log_line(
            ctx,
            format!("Warning: binary discovery failed; using fallback list ({message})"),
        );
    }

    remove_resolved_binaries(ctx, binaries)
}

pub(in crate::actions::install) fn remove_resolved_binaries(
    ctx: &mut ActionContext,
    binaries: Vec<String>,
) -> Result<()> {
    // Uninstall is destructive, so validate again immediately before building removal paths
    let binaries = validate_managed_binary_names(binaries)
        .with_context(|| "refusing to remove an unmanaged binary path")?;
    let expected_root = crate::actions::releases::entrypoint_target();
    for binary in binaries {
        let path = ctx.paths.bin_dir.join(binary);
        let removed = match std::fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                match remove_symlink_if_target(
                    &path,
                    &expected_root.join(path.file_name().unwrap_or_default()),
                )? {
                    RemoveSymlinkOutcome::Removed => true,
                    RemoveSymlinkOutcome::Missing => false,
                    RemoveSymlinkOutcome::TargetMismatch(actual) => {
                        return Err(anyhow!(
                            "refusing to remove unmanaged binary link {} -> {}",
                            path.display(),
                            actual.display()
                        ))
                    }
                }
            }
            Ok(metadata) if metadata.file_type().is_file() => {
                remove_regular_file(&path).with_context(|| "failed to remove legacy binary")?
            }
            Ok(_metadata) => {
                return Err(anyhow!(
                    "refusing to remove non-file binary entrypoint {}",
                    path.display()
                ))
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
            Err(error) => return Err(error).with_context(|| format!("inspect {}", path.display())),
        };
        if removed {
            log_line(ctx, format!("Removed binary {}", format_with_home(&path)));
        } else {
            log_line(
                ctx,
                format!("Binary not found at {}", format_with_home(&path)),
            );
        }
    }

    let install_root = ctx.paths.installed_release_root()?;
    if let Some(current_target) = read_symlink(&ctx.paths.installed_current_link()?)? {
        match remove_symlink_if_target(&ctx.paths.installed_current_link()?, &current_target)? {
            RemoveSymlinkOutcome::Removed | RemoveSymlinkOutcome::Missing => {}
            RemoveSymlinkOutcome::TargetMismatch(actual) => {
                return Err(anyhow!(
                    "current release link changed during uninstall to {}",
                    actual.display()
                ))
            }
        }
    }
    let pending = ctx.paths.installed_pending_manifest()?;
    if pending.exists() {
        remove_regular_file(&pending).context("remove pending release state")?;
    }
    if install_root.exists() {
        remove_directory_tree(&install_root).context("remove installed release generations")?;
    }

    Ok(())
}

fn resolve_release_dir(ctx: &mut ActionContext) -> Result<PathBuf> {
    if ctx.paths.is_release_archive() {
        // Bundled releases copy from the archive bin dir instead of Cargo target output
        // Keeping this branch here avoids leaking archive-specific paths into copy logic
        return Ok(ctx.paths.release_binary_dir());
    }

    // Ask cargo metadata for the target dir instead of assuming `target/release`
    let target_dir = resolve_target_directory(ctx.paths).with_context(|| {
        format!(
            "failed to resolve cargo target directory for {}",
            format_with_home(&ctx.paths.repo_root)
        )
    })?;
    Ok(target_dir.join("release"))
}
