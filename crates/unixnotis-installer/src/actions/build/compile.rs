//! Release build execution for installer-managed binaries

use anyhow::{anyhow, Result};

use super::super::{binaries::resolve_install_binaries, log_line, run_command, ActionContext};

pub(crate) fn run_build(ctx: &mut ActionContext) -> Result<()> {
    if ctx.paths.is_release_archive() {
        // Downloaded releases already ship binaries, so "build" becomes a bundle check
        return verify_release_binaries(ctx);
    }

    // Build release artifacts before copying them into the user bin directory
    log_line(ctx, "Building release binaries");

    // Resolve the managed binary list from installer metadata instead of guessing package names
    let binaries = resolve_install_binaries(ctx.paths)?;
    if binaries.is_empty() {
        return Err(anyhow!("no installable binaries discovered for build"));
    }

    // Build only the packages that installer metadata marked as installable
    let mut build = std::process::Command::new("cargo");
    build.args(["build", "--release"]);
    for binary in &binaries {
        // Pass each managed package explicitly so unrelated workspace crates stay out of the build
        build.args(["-p", binary]);
    }

    run_command(
        ctx,
        "cargo build --release",
        build,
        Some(&ctx.paths.repo_root),
    )?;
    Ok(())
}

fn verify_release_binaries(ctx: &mut ActionContext) -> Result<()> {
    log_line(ctx, "Using bundled release binaries");

    // The same resolver feeds build, install, and uninstall so the managed set cannot drift
    let binaries = resolve_install_binaries(ctx.paths)?;
    if binaries.is_empty() {
        return Err(anyhow!(
            "release manifest did not list installable binaries"
        ));
    }

    let missing = binaries
        .iter()
        // Release artifacts are copied from the archive bin folder during the install step
        .map(|binary| ctx.paths.release_binary_dir().join(binary))
        // Every listed file must exist before install starts changing user-owned state
        .filter(|path| !path.is_file())
        .map(|path| crate::paths::format_with_home(&path))
        .collect::<Vec<_>>();

    if !missing.is_empty() {
        return Err(anyhow!(
            "missing bundled release binaries: {}",
            missing.join(", ")
        ));
    }

    Ok(())
}

#[cfg(test)]
#[path = "tests/compile.rs"]
mod tests;
