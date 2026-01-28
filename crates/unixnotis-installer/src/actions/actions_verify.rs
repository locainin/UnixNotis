//! Build and verification actions for the installer.

use anyhow::{anyhow, Result};

use super::{actions_binaries::resolve_install_binaries, log_line, run_command, ActionContext};

pub fn run_verify_check(ctx: &mut ActionContext) -> Result<()> {
    log_line(ctx, "Running cargo check");
    let mut check = std::process::Command::new("cargo");
    check.arg("check").env("RUSTFLAGS", "-D warnings");
    run_command(ctx, "cargo check", check, Some(&ctx.paths.repo_root))?;
    Ok(())
}

pub fn run_verify_test(ctx: &mut ActionContext) -> Result<()> {
    log_line(ctx, "Running cargo test");
    let mut test = std::process::Command::new("cargo");
    test.arg("test").env("RUSTFLAGS", "-D warnings");
    run_command(ctx, "cargo test", test, Some(&ctx.paths.repo_root))?;
    Ok(())
}

pub fn run_verify_clippy(ctx: &mut ActionContext) -> Result<()> {
    log_line(ctx, "Running cargo clippy");
    let mut clippy = std::process::Command::new("cargo");
    clippy.args([
        "clippy",
        "--all-targets",
        "--all-features",
        "--",
        "-D",
        "warnings",
        "-W",
        "clippy::perf",
    ]);
    run_command(ctx, "cargo clippy", clippy, Some(&ctx.paths.repo_root))?;
    Ok(())
}

pub fn run_build(ctx: &mut ActionContext) -> Result<()> {
    log_line(ctx, "Building release binaries");
    let binaries = resolve_install_binaries(ctx.paths)?;
    if binaries.is_empty() {
        return Err(anyhow!("no installable binaries discovered for build"));
    }
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
