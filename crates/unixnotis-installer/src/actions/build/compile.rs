//! Release build execution for installer-managed binaries

use anyhow::{anyhow, Result};

use super::super::{
    actions_binaries::resolve_install_binaries, log_line, run_command, ActionContext,
};

pub(crate) fn run_build(ctx: &mut ActionContext) -> Result<()> {
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
