//! Environment checks for session requirements and tooling availability.

use std::env;
use std::fs::OpenOptions;
use std::path::Path;
use std::process::Command;

use crate::model::ActionMode;
use crate::paths::{format_with_home, InstallPaths};
use unixnotis_core::{
    gtk_css_features_from_version_string, program_in_path,
    GTK_CSS_CUSTOM_PROPERTIES_MIN_VERSION_LABEL,
};

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
    pub gtk4_css_features: CheckItem,
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

        // Modern CSS support is additive, so older GTK builds should warn instead of hard-fail
        let gtk4_css_features = match pkg_config_version("gtk4") {
            Ok(Some(version)) => match gtk_css_features_from_version_string(&version) {
                Some(features) if features.custom_properties => CheckItem::ok(
                    "GTK4 CSS features",
                    &format!("found {version}; modern css variables and var() are available"),
                ),
                Some(_) => CheckItem::warn(
                    "GTK4 CSS features",
                    &format!(
                        "found {version}; legacy theming still works, but modern css variables need {GTK_CSS_CUSTOM_PROPERTIES_MIN_VERSION_LABEL}"
                    ),
                ),
                None => CheckItem::warn(
                    "GTK4 CSS features",
                    &format!("found {version}; css feature level could not be parsed"),
                ),
            },
            Ok(None) if pkg_config.state == CheckState::Fail => CheckItem::warn(
                "GTK4 CSS features",
                "pkg-config missing; cannot probe GTK4 css feature level",
            ),
            Ok(None) => CheckItem::warn(
                "GTK4 CSS features",
                "pkg-config gtk4 not found; modern css feature support is unknown",
            ),
            Err(err) => CheckItem::warn("GTK4 CSS features", &format!("check failed: {err}")),
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
                let path_contains_bin = path_check_item(&paths);
                (install_paths, path_contains_bin)
            }
            Err(err) => (
                CheckItem::warn("Install paths", &format!("discovery failed: {err}")),
                CheckItem::warn("Shell PATH", "could not determine install bin path"),
            ),
        };

        Self {
            wayland,
            hyprland,
            systemd_user,
            cargo,
            pkg_config,
            gtk4_css_features,
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
    env::split_paths(&path_var).any(|entry| path_entries_match(&entry, &paths.bin_dir))
}

fn path_check_item(paths: &InstallPaths) -> CheckItem {
    let rendered_bin = format_with_home(&paths.bin_dir);
    // PATH alone is not enough here because uninstall can remove the command
    // while the shell still keeps the bin dir in its search path
    let path_ready = path_includes_bin(paths);
    // Check the managed install location so the status matches real command availability
    let command_installed = install_bin_has_command(paths, "noticenterctl");
    path_check_item_from(&rendered_bin, path_ready, command_installed)
}

fn install_bin_has_command(paths: &InstallPaths, command: &str) -> bool {
    // Check the managed install directory directly instead of relying on PATH search
    paths.bin_dir.join(command).is_file()
}

fn path_check_item_from(
    rendered_bin: &str,
    path_ready: bool,
    command_installed: bool,
) -> CheckItem {
    match (path_ready, command_installed) {
        // This is the only fully ready state for direct command use
        (true, true) => CheckItem::ok(
            "Shell PATH",
            &format!("includes {rendered_bin}; noticenterctl is installed there"),
        ),
        // Uninstall can leave PATH intact, so this still needs to warn
        (true, false) => CheckItem::warn(
            "Shell PATH",
            &format!(
                "includes {rendered_bin}, but noticenterctl is not installed there right now"
            ),
        ),
        // Install can finish before the current shell reloads its startup files
        (false, true) => CheckItem::warn(
            "Shell PATH",
            &format!(
                "missing {rendered_bin}; noticenterctl is installed there, but the current terminal session cannot run it directly until PATH is reloaded or a new terminal is opened"
            ),
        ),
        // Fresh systems hit this path before the first install
        (false, false) => CheckItem::warn(
            "Shell PATH",
            &format!(
                "missing {rendered_bin}; noticenterctl is not installed there right now"
            ),
        ),
    }
}

fn path_entries_match(entry: &Path, target: &Path) -> bool {
    if entry == target {
        return true;
    }

    match (entry.canonicalize(), target.canonicalize()) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use unixnotis_core::gtk_css_features_from_version_string;

    use super::{path_check_item_from, path_entries_match, CheckState};

    #[test]
    fn gtk_css_feature_parser_handles_major_and_minor_checks() {
        assert!(
            gtk_css_features_from_version_string("4.16.2")
                .expect("version")
                .custom_properties
        );
        assert!(
            gtk_css_features_from_version_string("4.18")
                .expect("version")
                .custom_properties
        );
        assert!(
            !gtk_css_features_from_version_string("4.14.9")
                .expect("version")
                .custom_properties
        );
        assert!(
            gtk_css_features_from_version_string("5.0.0")
                .expect("version")
                .custom_properties
        );
    }

    #[test]
    fn path_entries_match_accepts_exact_paths() {
        assert!(path_entries_match(
            Path::new("/tmp/unixnotis-bin"),
            Path::new("/tmp/unixnotis-bin")
        ));
    }

    #[test]
    fn shell_path_warns_when_bin_is_on_path_but_command_was_uninstalled() {
        let item = path_check_item_from("$HOME/.local/bin", true, false);

        assert_eq!(item.state, CheckState::Warn);
        assert!(item.detail.contains("noticenterctl is not installed there"));
    }

    #[test]
    fn shell_path_warns_when_fresh_shell_has_no_path_and_no_command() {
        let item = path_check_item_from("$HOME/.local/bin", false, false);

        assert_eq!(item.state, CheckState::Warn);
        assert!(item.detail.contains("missing $HOME/.local/bin"));
        assert!(item.detail.contains("noticenterctl is not installed there"));
    }

    #[test]
    fn shell_path_is_ok_only_when_path_and_command_are_both_present() {
        let item = path_check_item_from("$HOME/.local/bin", true, true);

        assert_eq!(item.state, CheckState::Ok);
        assert!(item.detail.contains("noticenterctl is installed there"));
    }
}
