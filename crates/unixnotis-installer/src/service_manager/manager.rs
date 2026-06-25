use std::path::{Path, PathBuf};

use super::{dinit, runit, systemd};
use super::{CommandSpec, ServiceArtifact};

#[cfg(test)]
pub const UNIXNOTIS_DAEMON_SERVICE: &str = systemd::SERVICE_NAME;
#[cfg(test)]
pub const UNIXNOTIS_DAEMON_DINIT_SERVICE: &str = dinit::SERVICE_NAME;
#[cfg(test)]
pub const UNIXNOTIS_DAEMON_RUNIT_SERVICE: &str = runit::SERVICE_NAME;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ServiceManagerKind {
    // User-level systemd remains the default backend for existing installs
    Systemd,
    // Dinit user mode uses service files plus boot.d dependency links
    Dinit,
    // Runit user mode uses a supervised service directory with an executable run script
    Runit,
}

impl ServiceManagerKind {
    fn label(self) -> &'static str {
        match self {
            Self::Systemd => "systemd --user",
            Self::Dinit => "dinit --user",
            Self::Runit => "runit user services",
        }
    }
}

pub struct ServiceManager {
    // Backend choice is stored beside paths so callers do not branch on path layout
    kind: ServiceManagerKind,
    // Root directory that receives backend-owned startup artifacts
    artifact_root: PathBuf,
}

impl ServiceManager {
    pub fn systemd_user(artifact_root: PathBuf) -> Self {
        Self {
            kind: ServiceManagerKind::Systemd,
            artifact_root,
        }
    }

    pub fn dinit_user(artifact_root: PathBuf) -> Self {
        Self {
            kind: ServiceManagerKind::Dinit,
            artifact_root,
        }
    }

    pub fn runit_user(artifact_root: PathBuf) -> Self {
        Self {
            kind: ServiceManagerKind::Runit,
            artifact_root,
        }
    }

    pub fn label(&self) -> &'static str {
        self.kind.label()
    }

    pub fn service_name(&self) -> &'static str {
        match self.kind {
            ServiceManagerKind::Systemd => systemd::SERVICE_NAME,
            ServiceManagerKind::Dinit => dinit::SERVICE_NAME,
            ServiceManagerKind::Runit => runit::SERVICE_NAME,
        }
    }

    pub fn artifact_label(&self) -> &'static str {
        match self.kind {
            ServiceManagerKind::Systemd => systemd::artifact_label(),
            ServiceManagerKind::Dinit => dinit::artifact_label(),
            ServiceManagerKind::Runit => runit::artifact_label(),
        }
    }

    pub fn manager_label(&self) -> &'static str {
        match self.kind {
            ServiceManagerKind::Systemd => systemd::manager_label(),
            ServiceManagerKind::Dinit => dinit::manager_label(),
            ServiceManagerKind::Runit => runit::manager_label(),
        }
    }

    pub fn artifact_root(&self) -> &Path {
        &self.artifact_root
    }

    pub fn primary_artifact_path(&self) -> PathBuf {
        match self.kind {
            ServiceManagerKind::Systemd => systemd::primary_artifact_path(&self.artifact_root),
            ServiceManagerKind::Dinit => dinit::primary_artifact_path(&self.artifact_root),
            ServiceManagerKind::Runit => runit::primary_artifact_path(&self.artifact_root),
        }
    }

    pub fn artifacts(&self, bin_dir: &Path) -> Vec<ServiceArtifact> {
        match self.kind {
            ServiceManagerKind::Systemd => systemd::artifacts(&self.artifact_root, bin_dir),
            ServiceManagerKind::Dinit => dinit::artifacts(&self.artifact_root, bin_dir),
            ServiceManagerKind::Runit => runit::artifacts(&self.artifact_root, bin_dir),
        }
    }

    pub fn availability_command(&self) -> Option<CommandSpec> {
        match self.kind {
            ServiceManagerKind::Systemd => systemd::availability_command(),
            ServiceManagerKind::Dinit => dinit::availability_command(),
            ServiceManagerKind::Runit => runit::availability_command(),
        }
    }

    pub fn is_enabled_command(&self) -> Option<CommandSpec> {
        match self.kind {
            ServiceManagerKind::Systemd => systemd::is_enabled_command(),
            ServiceManagerKind::Dinit => dinit::is_enabled_command(),
            ServiceManagerKind::Runit => runit::is_enabled_command(),
        }
    }

    pub fn enabled_by_artifacts(&self, _bin_dir: &Path) -> Option<bool> {
        match self.kind {
            ServiceManagerKind::Systemd => None,
            ServiceManagerKind::Dinit => Some(dinit::enabled_by_artifacts(&self.artifact_root)),
            ServiceManagerKind::Runit => Some(runit::enabled_by_artifacts(&self.artifact_root)),
        }
    }

    pub fn is_active_command(&self) -> Option<CommandSpec> {
        match self.kind {
            ServiceManagerKind::Systemd => systemd::is_active_command(),
            ServiceManagerKind::Dinit => dinit::is_active_command(),
            ServiceManagerKind::Runit => runit::is_active_command(&self.artifact_root),
        }
    }

    pub fn reload_after_artifact_change(&self) -> Option<CommandSpec> {
        match self.kind {
            ServiceManagerKind::Systemd => systemd::reload_after_artifact_change(),
            ServiceManagerKind::Dinit => dinit::reload_after_artifact_change(),
            ServiceManagerKind::Runit => runit::reload_after_artifact_change(),
        }
    }

    pub fn enable_now_command(&self) -> Option<CommandSpec> {
        match self.kind {
            ServiceManagerKind::Systemd => systemd::enable_now_command(),
            ServiceManagerKind::Dinit => dinit::enable_now_command(),
            ServiceManagerKind::Runit => runit::enable_now_command(&self.artifact_root),
        }
    }

    pub fn start_command(&self) -> Option<CommandSpec> {
        match self.kind {
            ServiceManagerKind::Systemd => systemd::start_command(),
            ServiceManagerKind::Dinit => dinit::start_command(),
            ServiceManagerKind::Runit => runit::start_command(&self.artifact_root),
        }
    }

    pub fn disable_now_command(&self) -> Option<CommandSpec> {
        match self.kind {
            ServiceManagerKind::Systemd => systemd::disable_now_command(),
            ServiceManagerKind::Dinit => dinit::disable_now_command(),
            ServiceManagerKind::Runit => runit::disable_now_command(&self.artifact_root),
        }
    }

    pub fn stop_for_reinstall_command(&self) -> Option<CommandSpec> {
        match self.kind {
            ServiceManagerKind::Systemd => systemd::stop_for_reinstall_command(),
            ServiceManagerKind::Dinit => dinit::stop_for_reinstall_command(),
            ServiceManagerKind::Runit => runit::stop_for_reinstall_command(&self.artifact_root),
        }
    }

    pub fn hyprland_startup_commands(&self, import_vars: &[&str]) -> Vec<String> {
        match self.kind {
            ServiceManagerKind::Systemd => systemd::hyprland_startup_commands(import_vars),
            ServiceManagerKind::Dinit => dinit::hyprland_startup_commands(import_vars),
            ServiceManagerKind::Runit => {
                runit::hyprland_startup_commands(&self.artifact_root, import_vars)
            }
        }
    }

    pub fn environment_sync_commands(
        &self,
        import_vars: &[(&str, String)],
        dbus_update_available: bool,
    ) -> Vec<CommandSpec> {
        match self.kind {
            ServiceManagerKind::Systemd => {
                systemd::environment_sync_commands(import_vars, dbus_update_available)
            }
            ServiceManagerKind::Dinit => dinit::environment_sync_commands(import_vars),
            ServiceManagerKind::Runit => runit::environment_sync_commands(),
        }
    }

    pub fn environment_sync_artifacts(
        &self,
        import_vars: &[(&str, String)],
    ) -> Vec<ServiceArtifact> {
        match self.kind {
            ServiceManagerKind::Systemd | ServiceManagerKind::Dinit => Vec::new(),
            ServiceManagerKind::Runit => {
                runit::environment_sync_artifacts(&self.artifact_root, import_vars)
            }
        }
    }

    pub fn readiness_warnings(&self) -> Vec<String> {
        match self.kind {
            ServiceManagerKind::Systemd | ServiceManagerKind::Runit => Vec::new(),
            ServiceManagerKind::Dinit => dinit::readiness_warnings(&self.artifact_root),
        }
    }
}
