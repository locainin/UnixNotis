//! Binary install and uninstall helpers

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use unixnotis_core::filesystem::copy_file_atomic;

use crate::managed_binaries::validate_managed_binary_names;
use crate::paths::format_with_home;

use super::super::{
    binaries::{
        resolve_install_binaries, resolve_install_binaries_best_effort, resolve_target_directory,
    },
    log_line, ActionContext,
};

pub fn install_binaries(ctx: &mut ActionContext) -> Result<()> {
    // Read the managed binary list from installer metadata so install and uninstall stay aligned
    let binaries = resolve_install_binaries(ctx.paths)?;
    // Cargo metadata is the only reliable way to find the active release target directory
    let release_dir = resolve_release_dir(ctx)?;

    // Check every source first so install never leaves a half-updated bin directory behind
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
    for binary in binaries {
        let source = release_dir.join(&binary);
        let destination = ctx.paths.bin_dir.join(&binary);
        // One helper handles both source builds and downloaded archives after source resolution
        copy_binary(ctx, &source, &destination)?;
    }

    Ok(())
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
    for binary in binaries {
        let path = ctx.paths.bin_dir.join(binary);
        if path.exists() {
            fs::remove_file(&path).with_context(|| "failed to remove binary")?;
            log_line(ctx, format!("Removed binary {}", format_with_home(&path)));
        } else {
            log_line(
                ctx,
                format!("Binary not found at {}", format_with_home(&path)),
            );
        }
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

fn copy_binary(ctx: &mut ActionContext, source: &Path, destination: &Path) -> Result<()> {
    if !source.exists() {
        return Err(anyhow!(
            "missing build artifact: {}",
            format_with_home(source)
        ));
    }

    let source_display = format_with_home(source);
    let destination_display = format_with_home(destination);
    // Core stages beside the destination and validates both paths through stable descriptors
    copy_file_atomic(source, destination).map_err(|err| {
        anyhow!("failed to install {source_display} -> {destination_display}: {err}")
    })?;
    log_line(
        ctx,
        format!(
            "Installed {} -> {}",
            source.file_name().unwrap_or_default().to_string_lossy(),
            format_with_home(destination)
        ),
    );
    Ok(())
}
