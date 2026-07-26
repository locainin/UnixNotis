//! Active systemd unit channel classification for source-install safety

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

use super::{log_line, ActionContext};

const SYSTEM_UNIT_ROOT: &str = "/usr/lib/systemd/user";
const SYSTEM_BINARY_ROOT: &str = "/usr/bin";
const MAX_SYSTEMCTL_OUTPUT_BYTES: usize = 32 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum InstallationChannel {
    HomeLocal,
    SystemPackage,
    Mixed,
    Unknown,
}

pub(super) fn reject_conflicting_installation_channel(ctx: &mut ActionContext) -> Result<()> {
    if !ctx.paths.service.is_systemd() {
        return Ok(());
    }
    let Some((fragment, executable)) = active_unit_paths()? else {
        return Ok(());
    };
    let channel = classify_installation_channel(
        &fragment,
        &executable,
        ctx.paths.service.artifact_root(),
        &ctx.paths.bin_dir,
    );
    match channel {
        InstallationChannel::HomeLocal => Ok(()),
        InstallationChannel::SystemPackage => {
            log_channel_conflict(ctx, "system package", &fragment, &executable);
            bail!(
                "the system-package UnixNotis installation must be removed with its package manager before a home-local install"
            )
        }
        InstallationChannel::Mixed => {
            log_channel_conflict(ctx, "mixed", &fragment, &executable);
            bail!(
                "mixed UnixNotis installation channels detected; repair the unit and executable paths before installing"
            )
        }
        InstallationChannel::Unknown => {
            log_channel_conflict(ctx, "unrecognized", &fragment, &executable);
            bail!(
                "the active UnixNotis unit uses an unrecognized installation channel; automatic replacement is unsafe"
            )
        }
    }
}

fn active_unit_paths() -> Result<Option<(PathBuf, PathBuf)>> {
    let mut command = crate::system_tools::command("systemctl")?;
    command.args([
        "--user",
        "show",
        "unixnotis-daemon.service",
        "--property=FragmentPath",
        "--property=ExecStart",
        "--no-pager",
    ]);
    let output = command
        .output()
        .context("inspect active UnixNotis systemd unit")?;
    if !output.status.success() {
        return Ok(None);
    }
    if output.stdout.len() > MAX_SYSTEMCTL_OUTPUT_BYTES {
        bail!("systemctl unit metadata exceeded the safe output limit");
    }
    let text = String::from_utf8(output.stdout).context("systemctl unit metadata was not UTF-8")?;
    let fragment = property_value(&text, "FragmentPath").map(PathBuf::from);
    let executable = property_value(&text, "ExecStart")
        .and_then(parse_exec_start_path)
        .map(PathBuf::from);
    match (fragment, executable) {
        (Some(fragment), Some(executable)) if !fragment.as_os_str().is_empty() => {
            Ok(Some((fragment, executable)))
        }
        (None, None) => Ok(None),
        _ => bail!("systemctl returned incomplete UnixNotis unit path metadata"),
    }
}

fn property_value<'a>(text: &'a str, name: &str) -> Option<&'a str> {
    text.lines()
        .find_map(|line| line.strip_prefix(name)?.strip_prefix('='))
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn parse_exec_start_path(value: &str) -> Option<&str> {
    let path = value
        .split(';')
        .find_map(|field| {
            field
                .trim()
                .trim_start_matches('{')
                .trim()
                .strip_prefix("path=")
        })?
        .trim();
    (!path.is_empty()).then_some(path)
}

fn classify_installation_channel(
    fragment: &Path,
    executable: &Path,
    home_unit_root: &Path,
    home_binary_root: &Path,
) -> InstallationChannel {
    let unit_channel = path_channel(fragment, home_unit_root, Path::new(SYSTEM_UNIT_ROOT));
    let binary_channel = path_channel(executable, home_binary_root, Path::new(SYSTEM_BINARY_ROOT));
    match (unit_channel, binary_channel) {
        (Some(InstallationChannel::HomeLocal), Some(InstallationChannel::HomeLocal)) => {
            InstallationChannel::HomeLocal
        }
        (Some(InstallationChannel::SystemPackage), Some(InstallationChannel::SystemPackage)) => {
            InstallationChannel::SystemPackage
        }
        (Some(_), Some(_)) => InstallationChannel::Mixed,
        _ => InstallationChannel::Unknown,
    }
}

fn path_channel(path: &Path, home_root: &Path, system_root: &Path) -> Option<InstallationChannel> {
    if path.starts_with(home_root) {
        Some(InstallationChannel::HomeLocal)
    } else if path.starts_with(system_root) {
        Some(InstallationChannel::SystemPackage)
    } else {
        None
    }
}

fn log_channel_conflict(ctx: &mut ActionContext, label: &str, fragment: &Path, executable: &Path) {
    log_line(
        ctx,
        format!("Error: {label} UnixNotis installation channel"),
    );
    log_line(ctx, format!("- unit: {}", fragment.display()));
    log_line(ctx, format!("- executable: {}", executable.display()));
}

#[cfg(test)]
#[path = "tests/installation_channel.rs"]
mod tests;
