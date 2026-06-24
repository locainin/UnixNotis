//! Environment checks for session requirements and tooling availability

mod gtk;
mod output;
mod shell;
mod system;

use crate::model::ActionMode;
use crate::paths::InstallPaths;

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
    pub service_manager: CheckItem,
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
        let wayland = system::wayland_check();
        let hyprland = system::hyprland_check();
        let service_manager = system::service_manager_check();
        let cargo = system::cargo_check();
        let pkg_config = system::pkg_config_check();
        let gtk4_css_features = gtk::gtk4_css_features_check(&pkg_config);
        let gtk4_layer_shell = gtk::gtk4_layer_shell_check(&pkg_config);
        let busctl = system::busctl_check();
        let dbus_update_env = system::dbus_update_env_check();

        let (install_paths, path_contains_bin) = match InstallPaths::discover() {
            Ok(paths) => {
                // Path discovery runs once so every later row reports the same install target
                let install_paths = system::install_paths_check(&paths);
                let path_contains_bin = shell::path_check_item(&paths);
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
            service_manager,
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
                // Trial mode only needs the runtime pieces required to launch from source
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
                // Install adds the writable path requirement on top of the runtime checks
                if self.wayland.state == CheckState::Fail {
                    return Err("Wayland session required".to_string());
                }
                if self.service_manager.state == CheckState::Fail {
                    return Err("supported service manager session required".to_string());
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
                // Uninstall still needs the active backend and writable paths to stop cleanly
                if self.service_manager.state == CheckState::Fail {
                    return Err("supported service manager session required".to_string());
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
