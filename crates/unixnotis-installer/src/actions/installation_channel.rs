//! Active systemd unit channel classification for source-install safety

use std::fs;
use std::io::ErrorKind;
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

#[derive(Debug, Eq, PartialEq)]
enum ActiveUnitMetadata {
    // No loaded unit leaves the home-local channel available
    Absent,
    // Session-only masks are safe to clear during an explicit installation
    RuntimeMasked,
    // Persistent masks reflect a user decision and must remain untouched
    PersistentMasked,
    // Loaded units retain both paths so mixed channels cannot pass unnoticed
    Paths {
        fragment: PathBuf,
        executable: PathBuf,
    },
}

pub(super) fn reject_conflicting_installation_channel(ctx: &mut ActionContext) -> Result<()> {
    if !ctx.paths.service.is_systemd() {
        return Ok(());
    }
    let metadata = active_unit_metadata()?;
    let paths = match metadata {
        ActiveUnitMetadata::Absent => return Ok(()),
        ActiveUnitMetadata::RuntimeMasked => {
            // A temporary mask can hide a package unit, so inspect its fixed artifacts first
            let Some(paths) = installed_system_package_paths_at(
                Path::new(SYSTEM_UNIT_ROOT),
                Path::new(SYSTEM_BINARY_ROOT),
            )?
            else {
                return Ok(());
            };
            paths
        }
        ActiveUnitMetadata::PersistentMasked => {
            bail!(
                "UnixNotis systemd unit is persistently masked; run `systemctl --user unmask unixnotis-daemon.service` before installing"
            )
        }
        ActiveUnitMetadata::Paths {
            fragment,
            executable,
        } => (fragment, executable),
    };
    reject_channel(ctx, &paths.0, &paths.1)
}

fn reject_channel(ctx: &mut ActionContext, fragment: &Path, executable: &Path) -> Result<()> {
    let channel = classify_installation_channel(
        fragment,
        executable,
        ctx.paths.service.artifact_root(),
        &ctx.paths.bin_dir,
    );
    match channel {
        InstallationChannel::HomeLocal => Ok(()),
        InstallationChannel::SystemPackage => {
            log_channel_conflict(ctx, "system package", fragment, executable);
            bail!(
                "the system-package UnixNotis installation must be removed with its package manager before a home-local install"
            )
        }
        InstallationChannel::Mixed => {
            log_channel_conflict(ctx, "mixed", fragment, executable);
            bail!(
                "mixed UnixNotis installation channels detected; repair the unit and executable paths before installing"
            )
        }
        InstallationChannel::Unknown => {
            log_channel_conflict(ctx, "unrecognized", fragment, executable);
            bail!(
                "the active UnixNotis unit uses an unrecognized installation channel; automatic replacement is unsafe"
            )
        }
    }
}

fn active_unit_metadata() -> Result<ActiveUnitMetadata> {
    let mut command = crate::system_tools::command("systemctl")?;
    command.args([
        "--user",
        "show",
        "unixnotis-daemon.service",
        "--property=LoadState",
        "--property=UnitFileState",
        "--property=FragmentPath",
        "--property=ExecStart",
        "--no-pager",
    ]);
    let output = command
        .output()
        .context("inspect active UnixNotis systemd unit")?;
    if !output.status.success() {
        return Ok(ActiveUnitMetadata::Absent);
    }
    if output.stdout.len() > MAX_SYSTEMCTL_OUTPUT_BYTES {
        bail!("systemctl unit metadata exceeded the safe output limit");
    }
    let text = String::from_utf8(output.stdout).context("systemctl unit metadata was not UTF-8")?;
    parse_active_unit_metadata(&text)
}

fn parse_active_unit_metadata(text: &str) -> Result<ActiveUnitMetadata> {
    match property_value(text, "LoadState") {
        Some("not-found") => return Ok(ActiveUnitMetadata::Absent),
        Some("masked") => {
            return if property_value(text, "UnitFileState") == Some("masked-runtime") {
                Ok(ActiveUnitMetadata::RuntimeMasked)
            } else {
                Ok(ActiveUnitMetadata::PersistentMasked)
            };
        }
        Some("loaded") => {}
        Some(state) => bail!("systemctl reported unusable UnixNotis unit load state {state}"),
        None => bail!("systemctl omitted UnixNotis unit load state metadata"),
    }

    let fragment = property_value(text, "FragmentPath").map(PathBuf::from);
    let executable = property_value(text, "ExecStart")
        .and_then(parse_exec_start_path)
        .map(PathBuf::from);
    match (fragment, executable) {
        (Some(fragment), Some(executable)) => Ok(ActiveUnitMetadata::Paths {
            fragment,
            executable,
        }),
        _ => bail!("systemctl returned incomplete UnixNotis unit path metadata"),
    }
}

fn installed_system_package_paths_at(
    unit_root: &Path,
    binary_root: &Path,
) -> Result<Option<(PathBuf, PathBuf)>> {
    // Fixed package locations remain visible even when systemd reports only a runtime mask
    let fragment = unit_root.join("unixnotis-daemon.service");
    let executable = binary_root.join("unixnotis-daemon");
    let fragment_exists = path_entry_exists(&fragment)?;
    let executable_exists = path_entry_exists(&executable)?;
    match (fragment_exists, executable_exists) {
        (false, false) => Ok(None),
        (true, true) => Ok(Some((fragment, executable))),
        _ => bail!(
            "incomplete system-package UnixNotis artifacts detected; repair or remove the package before installing"
        ),
    }
}

fn path_entry_exists(path: &Path) -> Result<bool> {
    // Metadata on the directory entry detects dangling links without following them
    match fs::symlink_metadata(path) {
        Ok(_metadata) => Ok(true),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error).with_context(|| format!("inspect {}", path.display())),
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
