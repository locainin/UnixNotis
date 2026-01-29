//! Environment checks for session requirements and tooling availability.

use std::env;
use std::fs::OpenOptions;
use std::path::Path;
use std::process::Command;

use crate::model::ActionMode;
use crate::paths::InstallPaths;
use unixnotis_core::program_in_path;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CheckState {
    Ok,
    Warn,
    Fail,
}

pub struct CheckItem {
    pub label: &'static str,
    pub state: CheckState,
    pub detail: String,
}

pub struct Checks {
    pub wayland: CheckItem,
    pub hyprland: CheckItem,
    pub systemd_user: CheckItem,
    pub cargo: CheckItem,
    pub pkg_config: CheckItem,
    pub gtk4_layer_shell: CheckItem,
    pub busctl: CheckItem,
    pub dbus_update_env: CheckItem,
    pub install_paths: CheckItem,
    pub path_contains_bin: CheckItem,
}

impl Checks {
    pub fn run() -> Self {
        let wayland_session = env::var("XDG_SESSION_TYPE")
            .map(|val| val == "wayland")
            .unwrap_or(false)
            || env::var("WAYLAND_DISPLAY")
                .map(|val| !val.is_empty())
                .unwrap_or(false);
        let runtime_ok = env::var("XDG_RUNTIME_DIR")
            .map(|val| !val.is_empty())
            .unwrap_or(false);
        // Align preflight with runtime requirements to avoid late install failures.
        let wayland = if wayland_session && runtime_ok {
            CheckItem::ok("Wayland", "session detected")
        } else if wayland_session {
            CheckItem::fail("Wayland", "session missing XDG_RUNTIME_DIR")
        } else {
            CheckItem::fail("Wayland", "session missing")
        };

        let hyprland = if env::var("HYPRLAND_INSTANCE_SIGNATURE").is_ok() {
            CheckItem::ok("Hyprland", "instance detected")
        } else {
            CheckItem::warn("Hyprland", "not detected")
        };

        let systemd_user = match command_success("systemctl", &["--user", "show-environment"]) {
            Ok(true) => CheckItem::ok("systemd --user", "session available"),
            Ok(false) => CheckItem::fail("systemd --user", "session unavailable"),
            Err(err) => CheckItem::fail("systemd --user", &format!("check failed: {err}")),
        };

        let cargo = match command_success("cargo", &["--version"]) {
            Ok(true) => CheckItem::ok("cargo", "available"),
            Ok(false) => CheckItem::fail("cargo", "not installed"),
            Err(err) => CheckItem::fail("cargo", &format!("check failed: {err}")),
        };

        let pkg_config = match command_success("pkg-config", &["--version"]) {
            Ok(true) => CheckItem::ok("pkg-config", "available"),
            Ok(false) => CheckItem::fail("pkg-config", "not installed"),
            Err(err) => CheckItem::fail("pkg-config", &format!("check failed: {err}")),
        };

        let gtk4_layer_shell = match pkg_config_version("gtk4-layer-shell-0") {
            Ok(Some(version)) => CheckItem::ok("gtk4-layer-shell", &format!("found {version}")),
            Ok(None) if pkg_config.state == CheckState::Fail => CheckItem::fail(
                "gtk4-layer-shell",
                "pkg-config missing; cannot probe gtk4-layer-shell",
            ),
            Ok(None) => CheckItem::fail(
                "gtk4-layer-shell",
                "pkg-config gtk4-layer-shell-0 not found; is gtk4-layer-shell installed?",
            ),
            Err(err) => CheckItem::fail("gtk4-layer-shell", &format!("check failed: {err}")),
        };

        let busctl = match command_success("busctl", &["--version"]) {
            Ok(true) => CheckItem::ok("busctl", "available"),
            Ok(false) => CheckItem::warn("busctl", "not found; owner detection limited"),
            Err(err) => CheckItem::warn("busctl", &format!("check failed: {err}")),
        };

        // Preflight keeps env sync failure from hiding until install steps run.
        let dbus_update_env = if program_in_path("dbus-update-activation-environment") {
            CheckItem::ok("dbus-update-activation-environment", "available")
        } else {
            CheckItem::warn(
                "dbus-update-activation-environment",
                "not found; session env may be stale",
            )
        };

        let (install_paths, path_contains_bin) = match InstallPaths::discover() {
            Ok(paths) => {
                let writable = install_paths_writable(&paths);
                let install_paths = if writable {
                    CheckItem::ok("Install paths", "writable")
                } else {
                    CheckItem::fail("Install paths", "not writable")
                };
                let in_path = path_includes_bin(&paths);
                let path_contains_bin = if in_path {
                    CheckItem::ok("PATH", "includes install bin")
                } else {
                    CheckItem::warn("PATH", "missing install bin")
                };
                (install_paths, path_contains_bin)
            }
            Err(err) => (
                CheckItem::warn("Install paths", &format!("discovery failed: {err}")),
                CheckItem::warn("PATH", "install bin unknown"),
            ),
        };

        Self {
            wayland,
            hyprland,
            systemd_user,
            cargo,
            pkg_config,
            gtk4_layer_shell,
            busctl,
            dbus_update_env,
            install_paths,
            path_contains_bin,
        }
    }

    pub fn ready_for(&self, mode: ActionMode) -> Result<(), String> {
        match mode {
            ActionMode::Test => {
                if self.wayland.state == CheckState::Fail {
                    return Err("Wayland session required".to_string());
                }
                if self.cargo.state == CheckState::Fail {
                    return Err("cargo is required for trial mode".to_string());
                }
                if self.gtk4_layer_shell.state == CheckState::Fail {
                    return Err(
                        "gtk4-layer-shell is required; is the gtk4-layer-shell package installed?"
                            .to_string(),
                    );
                }
            }
            ActionMode::Install => {
                if self.wayland.state == CheckState::Fail {
                    return Err("Wayland session required".to_string());
                }
                if self.systemd_user.state == CheckState::Fail {
                    return Err("systemd --user session required".to_string());
                }
                if self.cargo.state == CheckState::Fail {
                    return Err("cargo is required for installation".to_string());
                }
                if self.gtk4_layer_shell.state == CheckState::Fail {
                    return Err(
                        "gtk4-layer-shell is required; is the gtk4-layer-shell package installed?"
                            .to_string(),
                    );
                }
                if self.install_paths.state == CheckState::Fail {
                    return Err("install paths are not writable".to_string());
                }
            }
            ActionMode::Uninstall => {
                if self.systemd_user.state == CheckState::Fail {
                    return Err("systemd --user session required".to_string());
                }
                if self.install_paths.state == CheckState::Fail {
                    return Err("install paths are not writable".to_string());
                }
            }
            ActionMode::Reset => {}
        }
        Ok(())
    }
}

impl CheckItem {
    fn ok(label: &'static str, detail: &str) -> Self {
        Self {
            label,
            state: CheckState::Ok,
            detail: detail.to_string(),
        }
    }

    fn warn(label: &'static str, detail: &str) -> Self {
        Self {
            label,
            state: CheckState::Warn,
            detail: detail.to_string(),
        }
    }

    fn fail(label: &'static str, detail: &str) -> Self {
        Self {
            label,
            state: CheckState::Fail,
            detail: detail.to_string(),
        }
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

fn pkg_config_version(lib: &str) -> Result<Option<String>, String> {
    let output = Command::new("pkg-config")
        .args(["--modversion", lib])
        .output()
        .map_err(|err| err.to_string())?;
    if !output.status.success() {
        return Ok(None);
    }
    let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if version.is_empty() {
        Ok(None)
    } else {
        Ok(Some(version))
    }
}

fn install_paths_writable(paths: &InstallPaths) -> bool {
    // Validate both binary and unit directories because install/uninstall touches both.
    let bin_ok = path_is_writable(&paths.bin_dir);
    let unit_ok = path_is_writable(&paths.unit_dir);
    bin_ok && unit_ok
}

fn path_is_writable(path: &Path) -> bool {
    // Check write access using a temporary file in the directory or its parent.
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

fn path_includes_bin(paths: &InstallPaths) -> bool {
    // Confirm the install bin directory is in PATH for user convenience.
    let Ok(path_var) = env::var("PATH") else {
        return false;
    };
    let mut entries = env::split_paths(&path_var);
    entries.any(|entry| entry == paths.bin_dir)
}
