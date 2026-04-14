//! Build actions for the installer.

use anyhow::{anyhow, Result};

use super::{actions_binaries::resolve_install_binaries, log_line, run_command, ActionContext};

pub fn run_build(ctx: &mut ActionContext) -> Result<()> {
    // Build release artifacts before copying into the user bin directory
    log_line(ctx, "Building release binaries");
    let binaries = resolve_install_binaries(ctx.paths)?;
    if binaries.is_empty() {
        return Err(anyhow!("no installable binaries discovered for build"));
    }
    // Build only the binaries listed in installer metadata
    let mut build = std::process::Command::new("cargo");
    build.args(["build", "--release"]);
    for binary in &binaries {
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
