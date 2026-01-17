//! Environment checks for session requirements and tooling availability.

use std::env;
use std::process::Command;

use crate::model::ActionMode;

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
    pub gtk4_layer_shell: CheckItem,
    pub busctl: CheckItem,
}

impl Checks {
    pub fn run() -> Self {
        let wayland = if env::var("XDG_SESSION_TYPE")
            .map(|val| val == "wayland")
            .unwrap_or(false)
            || env::var("WAYLAND_DISPLAY")
                .map(|val| !val.is_empty())
                .unwrap_or(false)
        {
            CheckItem::ok("Wayland", "session detected")
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

        let gtk4_layer_shell = match pkg_config_version("gtk4-layer-shell-0") {
            Ok(Some(version)) => {
                CheckItem::ok("gtk4-layer-shell", &format!("found {version}"))
            }
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

        Self {
            wayland,
            hyprland,
            systemd_user,
            cargo,
            gtk4_layer_shell,
            busctl,
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
            }
            ActionMode::Uninstall => {
                if self.systemd_user.state == CheckState::Fail {
                    return Err("systemd --user session required".to_string());
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
