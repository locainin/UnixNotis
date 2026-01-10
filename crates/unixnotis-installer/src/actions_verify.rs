//! Build and verification actions for the installer.

use anyhow::Result;

use super::{log_line, run_command, ActionContext};

pub fn run_verify(ctx: &mut ActionContext) -> Result<()> {
    log_line(ctx, "Running cargo check/test/clippy");
    let mut check = std::process::Command::new("cargo");
    check.arg("check").env("RUSTFLAGS", "-D warnings");
    run_command(ctx, "cargo check", check, Some(&ctx.paths.repo_root))?;
    let mut test = std::process::Command::new("cargo");
    test.arg("test").env("RUSTFLAGS", "-D warnings");
    run_command(ctx, "cargo test", test, Some(&ctx.paths.repo_root))?;
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
    let mut build = std::process::Command::new("cargo");
    build.args([
        "build",
        "--release",
        "-p",
        "unixnotis-daemon",
        "-p",
        "unixnotis-popups",
        "-p",
        "unixnotis-center",
        "-p",
        "noticenterctl",
    ]);
    run_command(
        ctx,
        "cargo build --release",
        build,
        Some(&ctx.paths.repo_root),
    )?;
    Ok(())
}
