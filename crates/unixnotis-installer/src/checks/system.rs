//! Session and tool availability checks

use std::env;
use std::fs::OpenOptions;
use std::path::Path;
use std::process::Command;

use unixnotis_core::program_in_path;

use crate::paths::InstallPaths;

use super::CheckItem;

pub(super) fn wayland_check() -> CheckItem {
    let wayland_session = env::var("XDG_SESSION_TYPE")
        .map(|val| val == "wayland")
        .unwrap_or(false)
        || env::var("WAYLAND_DISPLAY")
            .map(|val| !val.is_empty())
            .unwrap_or(false);
    let runtime_ok = env::var("XDG_RUNTIME_DIR")
        .map(|val| !val.is_empty())
        .unwrap_or(false);
    // Wayland plus XDG runtime dir is the minimum runtime needed for the UI pieces
    if wayland_session && runtime_ok {
        CheckItem::ok("Wayland", "session detected")
    } else if wayland_session {
        CheckItem::fail("Wayland", "session missing XDG_RUNTIME_DIR")
    } else {
        CheckItem::fail("Wayland", "session missing")
    }
}

pub(super) fn hyprland_check() -> CheckItem {
    if env::var("HYPRLAND_INSTANCE_SIGNATURE").is_ok() {
        CheckItem::ok("Hyprland", "instance detected")
    } else {
        CheckItem::warn("Hyprland", "not detected")
    }
}

pub(super) fn systemd_user_check() -> CheckItem {
    match command_success("systemctl", &["--user", "show-environment"]) {
        Ok(true) => CheckItem::ok("systemd --user", "session available"),
        Ok(false) => CheckItem::fail("systemd --user", "session unavailable"),
        Err(err) => CheckItem::fail("systemd --user", &format!("check failed: {err}")),
    }
}

pub(super) fn cargo_check() -> CheckItem {
    match command_success("cargo", &["--version"]) {
        Ok(true) => CheckItem::ok("cargo", "available"),
        Ok(false) => CheckItem::fail("cargo", "not installed"),
        Err(err) => CheckItem::fail("cargo", &format!("check failed: {err}")),
    }
}

pub(super) fn pkg_config_check() -> CheckItem {
    match command_success("pkg-config", &["--version"]) {
        Ok(true) => CheckItem::ok("pkg-config", "available"),
        Ok(false) => CheckItem::fail("pkg-config", "not installed"),
        Err(err) => CheckItem::fail("pkg-config", &format!("check failed: {err}")),
    }
}

pub(super) fn busctl_check() -> CheckItem {
    match command_success("busctl", &["--version"]) {
        Ok(true) => CheckItem::ok("busctl", "available"),
        Ok(false) => CheckItem::warn("busctl", "not found; owner detection limited"),
        Err(err) => CheckItem::warn("busctl", &format!("check failed: {err}")),
    }
}

pub(super) fn dbus_update_env_check() -> CheckItem {
    if program_in_path("dbus-update-activation-environment") {
        CheckItem::ok("dbus-update-activation-environment", "available")
    } else {
        CheckItem::warn(
            "dbus-update-activation-environment",
            "not found; session env may be stale",
        )
    }
}

pub(super) fn install_paths_check(paths: &InstallPaths) -> CheckItem {
    if install_paths_writable(paths) {
        CheckItem::ok("Install paths", "writable")
    } else {
        CheckItem::fail("Install paths", "not writable")
    }
}

pub(super) fn pkg_config_version(lib: &str) -> Result<Option<String>, String> {
    let output = Command::new("pkg-config")
        .args(["--modversion", lib])
        .output()
        .map_err(|err| err.to_string())?;
    if !output.status.success() {
        // Missing pkg-config metadata is reported as None so callers can decide warn vs fail
        return Ok(None);
    }
    let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if version.is_empty() {
        Ok(None)
    } else {
        Ok(Some(version))
    }
}

fn command_success(program: &str, args: &[&str]) -> Result<bool, String> {
    Command::new(program)
        .args(args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|status| status.success())
        .map_err(|err| err.to_string())
}

fn install_paths_writable(paths: &InstallPaths) -> bool {
    // Validate both binary and unit directories because install and uninstall touch both
    let bin_ok = path_is_writable(&paths.bin_dir);
    let unit_ok = path_is_writable(&paths.unit_dir);
    bin_ok && unit_ok
}

fn path_is_writable(path: &Path) -> bool {
    // Probe the directory or its nearest existing parent with a temp file
    let mut target_dir = if path.exists() {
        path.to_path_buf()
    } else {
        match path.parent() {
            Some(parent) => parent.to_path_buf(),
            None => return false,
        }
    };
    while !target_dir.exists() {
        match target_dir.parent() {
            Some(parent) => target_dir = parent.to_path_buf(),
            None => return false,
        }
    }
    if !target_dir.is_dir() {
        return false;
    }
    let probe_name = format!(".unixnotis-installer-probe-{}", std::process::id());
    let probe_path = target_dir.join(probe_name);
    let result = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&probe_path);
    if result.is_err() {
        return false;
    }
    let _ = std::fs::remove_file(&probe_path);
    true
}
