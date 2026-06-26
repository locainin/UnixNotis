//! Binary install and uninstall helpers

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};

use crate::paths::format_with_home;

use super::super::{
    binaries::{
        resolve_install_binaries, resolve_install_binaries_best_effort, resolve_target_directory,
    },
    log_line, ActionContext,
};

pub(crate) fn install_binaries(ctx: &mut ActionContext) -> Result<()> {
    // Read the managed binary list from installer metadata so install and uninstall stay aligned
    let binaries = resolve_install_binaries(ctx.paths)?;
    // Cargo metadata is the only reliable way to find the active release target directory
    let release_dir = resolve_release_dir(ctx)?;

    fs::create_dir_all(&ctx.paths.bin_dir).with_context(|| "failed to create bin directory")?;

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

    for binary in binaries {
        let source = release_dir.join(&binary);
        let destination = ctx.paths.bin_dir.join(&binary);
        copy_binary(ctx, &source, &destination)?;
    }

    Ok(())
}

pub(crate) fn remove_binaries(ctx: &mut ActionContext) -> Result<()> {
    // Best-effort discovery keeps uninstall usable even when the workspace is partially broken
    let (binaries, warning) = resolve_install_binaries_best_effort(ctx.paths);
    if let Some(message) = warning {
        log_line(
            ctx,
            format!(
                "Warning: binary discovery failed; using fallback list ({})",
                message
            ),
        );
    }

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
    // Stage the copy beside the final file so the rename can replace atomically
    let temp_name = format!(
        "{}.tmp-{}",
        destination
            .file_name()
            .unwrap_or_default()
            .to_string_lossy(),
        std::process::id()
    );
    let temp_path = destination.with_file_name(temp_name);

    if temp_path.exists() {
        // Clear stale temp files from interrupted installs before staging a new copy
        fs::remove_file(&temp_path).with_context(|| "failed to remove stale temp file")?;
    }

    fs::copy(source, &temp_path).map_err(|err| {
        anyhow!(
            "failed to stage {} -> {}: {}",
            source_display,
            format_with_home(&temp_path),
            err
        )
    })?;

    // Rename replaces the destination in one step so there is no missing-binary window
    if let Err(err) = fs::rename(&temp_path, destination) {
        let _ = fs::remove_file(&temp_path);
        return Err(anyhow!(
            "failed to install {} -> {}: {}",
            source_display,
            destination_display,
            err
        ));
    }
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
