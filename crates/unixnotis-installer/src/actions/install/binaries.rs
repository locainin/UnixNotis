//! Binary install and uninstall helpers

use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};

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
    // Stage the copy beside the final file so the rename can replace atomically
    let temp_path = stage_binary_copy_with_retry(source, destination).map_err(|err| {
        anyhow!("failed to stage {source_display} -> {destination_display}: {err}")
    })?;

    // Rename replaces the destination in one step so there is no missing-binary window
    if let Err(err) = fs::rename(&temp_path, destination) {
        let _ = fs::remove_file(&temp_path);
        return Err(anyhow!(
            "failed to install {source_display} -> {destination_display}: {err}"
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

pub(super) fn binary_temp_path(destination: &Path) -> PathBuf {
    // The temp file sits beside the final binary so rename stays atomic
    let temp_name = format!(
        "{}.tmp-{}",
        destination
            .file_name()
            .unwrap_or_default()
            .to_string_lossy(),
        std::process::id()
    );
    destination.with_file_name(temp_name)
}

fn stage_binary_copy(source: &Path, temp_path: &Path) -> io::Result<()> {
    // create_new refuses attacker-created symlinks or stale files at the temp path
    let mut input = File::open(source)?;
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(temp_path)?;
    io::copy(&mut input, &mut output).inspect_err(|_err| {
        let _ = fs::remove_file(temp_path);
    })?;
    output.sync_all().inspect_err(|_err| {
        let _ = fs::remove_file(temp_path);
    })?;
    let permissions = fs::metadata(source)?.permissions();
    fs::set_permissions(temp_path, permissions).inspect_err(|_err| {
        let _ = fs::remove_file(temp_path);
    })
}

pub(in crate::actions::install) fn stage_binary_copy_with_retry(
    source: &Path,
    destination: &Path,
) -> io::Result<PathBuf> {
    for attempt in 0..16 {
        let temp_path = binary_temp_path_attempt(destination, attempt);
        match stage_binary_copy(source, &temp_path) {
            Ok(()) => return Ok(temp_path),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "could not allocate a safe temporary binary path",
    ))
}

pub(in crate::actions::install) fn binary_temp_path_attempt(
    destination: &Path,
    attempt: u8,
) -> PathBuf {
    if attempt == 0 {
        return binary_temp_path(destination);
    }
    let file_name = destination
        .file_name()
        .unwrap_or_default()
        .to_string_lossy();
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock moved backwards")
        .as_nanos();
    destination.with_file_name(format!(
        "{file_name}.tmp-{}-{nonce}-{attempt}",
        std::process::id()
    ))
}
