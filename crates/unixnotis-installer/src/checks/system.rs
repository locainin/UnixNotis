//! Session and tool availability checks

use std::env;
use std::fs::OpenOptions;
use std::path::Path;
use std::process::Command;

use unixnotis_core::program_in_path;

use crate::paths::{InstallPaths, ServiceManagerChoice};
use crate::service_manager::{CommandSpec, ReadinessIssue, ServiceManager};

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

pub(super) fn service_manager_check(service_manager: Option<ServiceManagerChoice>) -> CheckItem {
    let Ok(paths) = InstallPaths::discover_with_service_manager(service_manager) else {
        return CheckItem::fail("Service manager", "install paths unavailable");
    };
    service_manager_check_from(&paths.service)
}

fn service_manager_check_from(manager: &ServiceManager) -> CheckItem {
    let issues = manager.readiness_issues();
    if let Some(detail) = readiness_error_detail(&issues) {
        // Hard readiness errors are shown before running optional availability probes
        return CheckItem::fail("Service manager", &detail);
    }
    if let Some(spec) = manager.availability_command() {
        // Backends with a native availability command still report softer setup warnings
        return availability_check_item(manager, &spec, &issues);
    }
    if let Some(detail) = readiness_warning_detail(manager, &issues) {
        // Some experimental backends have no global probe, so warnings become the check result
        return CheckItem::warn("Service manager", &detail);
    }
    // Some managers have no cheap global probe, so backend readiness is the availability check
    CheckItem::ok("Service manager", &format!("{} ready", manager.label()))
}

fn availability_check_item(
    manager: &ServiceManager,
    spec: &CommandSpec,
    issues: &[ReadinessIssue],
) -> CheckItem {
    match spec.to_command().status() {
        Ok(status) if status.success() => match readiness_warning_detail(manager, issues) {
            // A manager can be available while still needing user setup for autostart
            Some(detail) => CheckItem::warn("Service manager", &detail),
            None => CheckItem::ok("Service manager", &format!("{} available", manager.label())),
        },
        Ok(_) => CheckItem::fail(
            "Service manager",
            &format!("{} unavailable", manager.label()),
        ),
        Err(err) => CheckItem::fail("Service manager", &format!("check failed: {err}")),
    }
}

pub(super) fn readiness_error_detail(issues: &[ReadinessIssue]) -> Option<String> {
    // Errors are returned without a backend prefix because they are already specific
    let errors = readiness_messages(issues, true);
    (!errors.is_empty()).then(|| errors.join("; "))
}

pub(super) fn readiness_warning_detail(
    manager: &ServiceManager,
    issues: &[ReadinessIssue],
) -> Option<String> {
    // Warning detail names the backend so the setup hint is not detached from context
    let warnings = readiness_messages(issues, false);
    (!warnings.is_empty()).then(|| {
        format!(
            "{} ready with warnings: {}",
            manager.label(),
            warnings.join("; ")
        )
    })
}

pub(super) fn readiness_messages(issues: &[ReadinessIssue], errors: bool) -> Vec<String> {
    issues
        .iter()
        // Severity filtering is kept pure for direct unit coverage
        .filter(|issue| issue.is_error() == errors)
        .map(|issue| issue.message().to_string())
        .collect()
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
    // Validate both binary and service directories because install and uninstall touch both
    let bin_ok = path_is_writable(&paths.bin_dir);
    let service_ok = path_is_writable(paths.service.artifact_root());
    bin_ok && service_ok
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
