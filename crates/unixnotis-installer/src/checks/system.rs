//! Session and tool availability checks

use std::env;
use std::path::Path;

use crate::paths::{InstallPaths, ServiceManagerChoice};
use crate::service_manager::contract::{ServiceManagerAvailability, ServiceProbeState};
use crate::service_manager::{ReadinessIssue, ServiceManager};
use crate::system_tools;
use crate::toolchain::resolve_cargo;
use unixnotis_core::filesystem::{remove_regular_file, write_file_if_missing};

use super::CheckItem;

pub(super) fn wayland_check() -> CheckItem {
    let wayland_session = env::var("XDG_SESSION_TYPE").is_ok_and(|val| val == "wayland")
        || env::var("WAYLAND_DISPLAY").is_ok_and(|val| !val.is_empty());
    let runtime_ok = env::var("XDG_RUNTIME_DIR").is_ok_and(|val| !val.is_empty());
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

pub(super) fn service_manager_check_from(manager: &ServiceManager) -> CheckItem {
    let issues = manager.readiness_issues();
    if let Some(detail) = readiness_error_detail(&issues) {
        // Hard readiness errors are shown before running optional availability probes
        return CheckItem::fail("Service manager", &detail);
    }
    // One semantic interpreter is shared with conflict detection and activation checks
    match manager.availability_state() {
        Ok(Some(ServiceManagerAvailability::Available)) => {
            available_manager_check_item(manager, &issues)
        }
        Ok(Some(ServiceManagerAvailability::Unavailable)) => CheckItem::fail(
            "Service manager",
            &format!("{} unavailable", manager.label()),
        ),
        Ok(Some(ServiceManagerAvailability::Indeterminate)) => CheckItem::fail(
            "Service manager",
            &format!("{} availability is indeterminate", manager.label()),
        ),
        Ok(None) => native_service_probe_check_item(manager, &issues),
        Err(err) => CheckItem::fail("Service manager", &format!("check failed: {err}")),
    }
}

fn available_manager_check_item(manager: &ServiceManager, issues: &[ReadinessIssue]) -> CheckItem {
    readiness_warning_detail(manager, issues).map_or_else(
        || CheckItem::ok("Service manager", &format!("{} available", manager.label())),
        |detail| CheckItem::warn("Service manager", &detail),
    )
}

fn native_service_probe_check_item(
    manager: &ServiceManager,
    issues: &[ReadinessIssue],
) -> CheckItem {
    // Runit and s6 have no separate manager transport query, so their bounded service probe
    // decides whether the selected backend can be inspected without inventing another contract
    match manager.active_probe().evaluate_state() {
        Ok(ServiceProbeState::Absent | ServiceProbeState::Inactive | ServiceProbeState::Active) => {
            readiness_warning_detail(manager, issues).map_or_else(
                || CheckItem::ok("Service manager", &format!("{} ready", manager.label())),
                |detail| CheckItem::warn("Service manager", &detail),
            )
        }
        Ok(ServiceProbeState::Unavailable) => CheckItem::fail(
            "Service manager",
            &format!("{} unavailable", manager.label()),
        ),
        Ok(ServiceProbeState::Indeterminate) => CheckItem::fail(
            "Service manager",
            &format!("{} state is indeterminate", manager.label()),
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

pub(super) fn cargo_check(release_archive: bool) -> CheckItem {
    if release_archive {
        // Downloaded releases install bundled binaries and do not need a Rust toolchain
        // Source checkouts still require cargo because the installer builds before copying
        return CheckItem::ok("cargo", "not required for release archive");
    }

    match resolve_cargo() {
        Ok(_) => CheckItem::ok("cargo", "available"),
        Err(_) => CheckItem::fail("cargo", "not installed in approved toolchain locations"),
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

pub(super) fn dbus_update_env_check(manager: Option<&ServiceManager>) -> CheckItem {
    if system_tools::program_exists("dbus-update-activation-environment") {
        CheckItem::ok("dbus-update-activation-environment", "available")
    } else if manager.is_some_and(|manager| !manager.uses_dbus_environment_helper()) {
        CheckItem::ok(
            "dbus-update-activation-environment",
            "not required for selected service manager",
        )
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
    let output = system_tools::command("pkg-config")
        .map_err(|err| err.to_string())?
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

pub(super) fn command_success(program: &str, args: &[&str]) -> Result<bool, String> {
    system_tools::command(program)
        .map_err(|err| err.to_string())?
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
    match write_file_if_missing(&probe_path, b"", 0o600) {
        Ok(true) => {
            // Cleanup must succeed before the directory is reported as writable
            remove_regular_file(&probe_path).is_ok_and(|removed| removed)
        }
        Ok(false) | Err(_) => false,
    }
}

#[cfg(test)]
#[path = "tests/system.rs"]
mod tests;
