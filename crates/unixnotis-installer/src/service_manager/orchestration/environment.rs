//! Session startup and environment propagation dispatch

use super::super::backends::{dinit, runit, s6, systemd};
use super::super::contract::{CommandSpec, ServiceArtifact};
use super::model::{ServiceManager, ServiceManagerKind};

impl ServiceManager {
    pub fn hyprland_startup_commands(&self, import_vars: &[&str]) -> Vec<String> {
        // Startup lines mirror the selected manager instead of assuming systemd
        match self.kind {
            ServiceManagerKind::Systemd => systemd::hyprland_startup_commands(import_vars),
            ServiceManagerKind::Dinit => dinit::hyprland_startup_commands(import_vars),
            ServiceManagerKind::Runit => {
                runit::hyprland_startup_commands(&self.artifact_root, import_vars)
            }
            ServiceManagerKind::S6 => {
                s6::hyprland_startup_commands(&self.artifact_root, self.live_root(), import_vars)
            }
        }
    }

    pub fn environment_sync_commands(
        &self,
        import_vars: &[(&str, String)],
        dbus_update_available: bool,
    ) -> Vec<CommandSpec> {
        // Command-backed managers use argv while artifact-backed managers return no command
        match self.kind {
            ServiceManagerKind::Systemd => {
                systemd::environment_sync_commands(import_vars, dbus_update_available)
            }
            ServiceManagerKind::Dinit => dinit::environment_sync_commands(import_vars),
            ServiceManagerKind::Runit => runit::environment_sync_commands(),
            ServiceManagerKind::S6 => s6::environment_sync_commands(),
        }
    }

    pub fn environment_sync_artifacts(
        &self,
        import_var_names: &[&str],
        import_vars: &[(&str, String)],
    ) -> Vec<ServiceArtifact> {
        // Runit and s6 need envdir artifacts because their control tools do not import environment
        match self.kind {
            ServiceManagerKind::Systemd | ServiceManagerKind::Dinit => Vec::new(),
            ServiceManagerKind::Runit => runit::environment_sync_artifacts(
                &self.artifact_root,
                import_var_names,
                import_vars,
            ),
            ServiceManagerKind::S6 => {
                s6::environment_sync_artifacts(&self.artifact_root, import_var_names, import_vars)
            }
        }
    }

    pub const fn uses_dbus_environment_helper(&self) -> bool {
        // Other managers use native import commands or envdir artifacts
        matches!(self.kind, ServiceManagerKind::Systemd)
    }
}
