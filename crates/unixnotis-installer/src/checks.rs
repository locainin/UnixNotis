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

        let systemd_user = if command_success("systemctl", &["--user", "show-environment"]) {
            CheckItem::ok("systemd --user", "session available")
        } else {
            CheckItem::fail("systemd --user", "session unavailable")
        };

        let cargo = if command_success("cargo", &["--version"]) {
            CheckItem::ok("cargo", "available")
        } else {
            CheckItem::fail("cargo", "not installed")
        };

        let busctl = if command_success("busctl", &["--version"]) {
            CheckItem::ok("busctl", "available")
        } else {
            CheckItem::warn("busctl", "not found; owner detection limited")
        };

        Self {
            wayland,
            hyprland,
            systemd_user,
            cargo,
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
            }
            ActionMode::Uninstall => {
                if self.systemd_user.state == CheckState::Fail {
                    return Err("systemd --user session required".to_string());
                }
            }
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

fn command_success(program: &str, args: &[&str]) -> bool {
    Command::new(program)
        .args(args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}
