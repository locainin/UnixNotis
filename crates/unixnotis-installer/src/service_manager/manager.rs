use std::path::{Path, PathBuf};

use super::{dinit, runit, s6, systemd};
use super::{CommandSpec, ServiceArtifact, ServiceProbe};

// Test exports keep exact legacy names visible without making them production API
#[cfg(test)]
pub const UNIXNOTIS_DAEMON_SERVICE: &str = systemd::SERVICE_NAME;
#[cfg(test)]
pub const UNIXNOTIS_DAEMON_DINIT_SERVICE: &str = dinit::SERVICE_NAME;
#[cfg(test)]
pub const UNIXNOTIS_DAEMON_RUNIT_SERVICE: &str = runit::SERVICE_NAME;
#[cfg(test)]
pub const UNIXNOTIS_DAEMON_S6_SERVICE: &str = s6::SERVICE_NAME;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ServiceManagerKind {
    // User-level systemd remains the default backend for existing installs
    Systemd,
    // Dinit user mode uses service files plus boot.d dependency links
    Dinit,
    // Runit user mode uses a supervised service directory with an executable run script
    Runit,
    // s6 user mode uses source directories plus a local s6-rc live database
    S6,
}

impl ServiceManagerKind {
    fn label(self) -> &'static str {
        match self {
            Self::Systemd => "systemd --user",
            Self::Dinit => "dinit --user",
            Self::Runit => "runit user services",
            Self::S6 => "s6-rc user services",
        }
    }
}

pub struct ServiceManager {
    // Backend choice is stored beside paths so callers do not branch on path layout
    kind: ServiceManagerKind,
    // Root directory receives only artifacts owned by this backend
    artifact_root: PathBuf,
    // Some managers separate persistent source definitions from live control sockets/state
    live_root: Option<PathBuf>,
}

impl ServiceManager {
    pub fn systemd_user(artifact_root: PathBuf) -> Self {
        Self {
            kind: ServiceManagerKind::Systemd,
            artifact_root,
            live_root: None,
        }
    }

    pub fn dinit_user(artifact_root: PathBuf) -> Self {
        Self {
            kind: ServiceManagerKind::Dinit,
            artifact_root,
            live_root: None,
        }
    }

    pub fn runit_user(artifact_root: PathBuf) -> Self {
        Self {
            kind: ServiceManagerKind::Runit,
            artifact_root,
            live_root: None,
        }
    }

    pub fn s6_user(artifact_root: PathBuf, live_root: PathBuf) -> Self {
        Self {
            kind: ServiceManagerKind::S6,
            artifact_root,
            live_root: Some(live_root),
        }
    }

    pub fn label(&self) -> &'static str {
        // Labels are used in user-facing checks and logs, not command construction
        self.kind.label()
    }

    pub fn service_name(&self) -> &'static str {
        // Backends can choose names that match their manager's normal service naming
        match self.kind {
            ServiceManagerKind::Systemd => systemd::SERVICE_NAME,
            ServiceManagerKind::Dinit => dinit::SERVICE_NAME,
            ServiceManagerKind::Runit => runit::SERVICE_NAME,
            ServiceManagerKind::S6 => s6::SERVICE_NAME,
        }
    }

    pub fn artifact_label(&self) -> &'static str {
        match self.kind {
            ServiceManagerKind::Systemd => systemd::artifact_label(),
            ServiceManagerKind::Dinit => dinit::artifact_label(),
            ServiceManagerKind::Runit => runit::artifact_label(),
            ServiceManagerKind::S6 => s6::artifact_label(),
        }
    }

    pub fn manager_label(&self) -> &'static str {
        match self.kind {
            ServiceManagerKind::Systemd => systemd::manager_label(),
            ServiceManagerKind::Dinit => dinit::manager_label(),
            ServiceManagerKind::Runit => runit::manager_label(),
            ServiceManagerKind::S6 => s6::manager_label(),
        }
    }

    pub fn artifact_root(&self) -> &Path {
        // Exposed for installer summaries and tests, not for ad hoc writes
        &self.artifact_root
    }

    pub fn primary_artifact_path(&self) -> PathBuf {
        // Summaries need one stable path even when the backend owns several artifacts
        match self.kind {
            ServiceManagerKind::Systemd => systemd::primary_artifact_path(&self.artifact_root),
            ServiceManagerKind::Dinit => dinit::primary_artifact_path(&self.artifact_root),
            ServiceManagerKind::Runit => runit::primary_artifact_path(&self.artifact_root),
            ServiceManagerKind::S6 => s6::primary_artifact_path(&self.artifact_root),
        }
    }

    pub fn artifacts(&self, bin_dir: &Path) -> Vec<ServiceArtifact> {
        // Artifact rendering is backend-owned so non-systemd managers are not unit-shaped
        match self.kind {
            ServiceManagerKind::Systemd => systemd::artifacts(&self.artifact_root, bin_dir),
            ServiceManagerKind::Dinit => dinit::artifacts(&self.artifact_root, bin_dir),
            ServiceManagerKind::Runit => runit::artifacts(&self.artifact_root, bin_dir),
            ServiceManagerKind::S6 => s6::artifacts(&self.artifact_root, bin_dir),
        }
    }

    pub fn availability_command(&self) -> Option<CommandSpec> {
        // Availability probes should be cheap and safe to run before install
        match self.kind {
            ServiceManagerKind::Systemd => systemd::availability_command(),
            ServiceManagerKind::Dinit => dinit::availability_command(),
            ServiceManagerKind::Runit => runit::availability_command(),
            ServiceManagerKind::S6 => s6::availability_command(),
        }
    }

    pub fn is_enabled_command(&self) -> Option<CommandSpec> {
        // Only managers with a native enabled-state command return one here
        match self.kind {
            ServiceManagerKind::Systemd => systemd::is_enabled_command(),
            ServiceManagerKind::Dinit => dinit::is_enabled_command(),
            ServiceManagerKind::Runit => runit::is_enabled_command(),
            ServiceManagerKind::S6 => s6::is_enabled_command(),
        }
    }

    pub fn enabled_by_artifacts(&self) -> Option<bool> {
        // Artifact-backed managers treat installer-owned files as persistent enablement
        match self.kind {
            ServiceManagerKind::Systemd => None,
            ServiceManagerKind::Dinit => Some(dinit::enabled_by_artifacts(&self.artifact_root)),
            ServiceManagerKind::Runit => Some(runit::enabled_by_artifacts(&self.artifact_root)),
            ServiceManagerKind::S6 => Some(s6::enabled_by_artifacts(&self.artifact_root)),
        }
    }

    pub fn active_probe(&self) -> Option<ServiceProbe> {
        match self.kind {
            ServiceManagerKind::Systemd => {
                systemd::is_active_command().map(ServiceProbe::exit_status)
            }
            ServiceManagerKind::Dinit => dinit::is_active_command().map(ServiceProbe::exit_status),
            ServiceManagerKind::Runit => runit::active_probe(&self.artifact_root),
            ServiceManagerKind::S6 => s6::active_probe(self.live_root()),
        }
    }

    pub fn reload_after_artifact_change(&self) -> Option<CommandSpec> {
        // Reload is optional because several managers discover artifacts on start
        match self.kind {
            ServiceManagerKind::Systemd => systemd::reload_after_artifact_change(),
            ServiceManagerKind::Dinit => dinit::reload_after_artifact_change(),
            ServiceManagerKind::Runit => runit::reload_after_artifact_change(),
            ServiceManagerKind::S6 => s6::reload_after_artifact_change(),
        }
    }

    pub fn enable_now_command(&self) -> Option<CommandSpec> {
        match self.kind {
            ServiceManagerKind::Systemd => systemd::enable_now_command(),
            ServiceManagerKind::Dinit => dinit::enable_now_command(),
            ServiceManagerKind::Runit => runit::enable_now_command(&self.artifact_root),
            ServiceManagerKind::S6 => s6::enable_now_command(self.live_root()),
        }
    }

    pub fn start_command(&self) -> Option<CommandSpec> {
        match self.kind {
            ServiceManagerKind::Systemd => systemd::start_command(),
            ServiceManagerKind::Dinit => dinit::start_command(),
            ServiceManagerKind::Runit => runit::start_command(&self.artifact_root),
            ServiceManagerKind::S6 => s6::start_command(self.live_root()),
        }
    }

    pub fn disable_now_command(&self) -> Option<CommandSpec> {
        match self.kind {
            ServiceManagerKind::Systemd => systemd::disable_now_command(),
            ServiceManagerKind::Dinit => dinit::disable_now_command(),
            ServiceManagerKind::Runit => runit::disable_now_command(&self.artifact_root),
            ServiceManagerKind::S6 => s6::disable_now_command(self.live_root()),
        }
    }

    pub fn stop_for_reinstall_command(&self) -> Option<CommandSpec> {
        match self.kind {
            ServiceManagerKind::Systemd => systemd::stop_for_reinstall_command(),
            ServiceManagerKind::Dinit => dinit::stop_for_reinstall_command(),
            ServiceManagerKind::Runit => runit::stop_for_reinstall_command(&self.artifact_root),
            ServiceManagerKind::S6 => s6::stop_for_reinstall_command(self.live_root()),
        }
    }

    pub fn hyprland_startup_commands(&self, import_vars: &[&str]) -> Vec<String> {
        // Session bootstrap belongs to the same backend that owns daemon lifecycle commands
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
        // Command-backed imports use argv only; artifact-backed imports return no command
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
        // Runit needs envdir files because sv does not import environment into runsv
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

    pub fn pre_start_artifacts_to_remove(&self) -> Vec<ServiceArtifact> {
        // Runit starts watched service directories immediately unless a down file is present
        match self.kind {
            ServiceManagerKind::Systemd | ServiceManagerKind::Dinit | ServiceManagerKind::S6 => {
                Vec::new()
            }
            ServiceManagerKind::Runit => runit::pre_start_artifacts_to_remove(&self.artifact_root),
        }
    }

    pub fn pre_start_artifacts_to_write(&self) -> Vec<ServiceArtifact> {
        // Start gates are temporary files and are not part of steady install state
        match self.kind {
            ServiceManagerKind::Systemd | ServiceManagerKind::Dinit | ServiceManagerKind::S6 => {
                Vec::new()
            }
            ServiceManagerKind::Runit => runit::pre_start_artifacts_to_write(&self.artifact_root),
        }
    }

    pub fn readiness_warnings(&self) -> Vec<String> {
        // Readiness warnings are advisory and must never rewrite user-owned manager config
        match self.kind {
            ServiceManagerKind::Systemd => Vec::new(),
            ServiceManagerKind::Dinit => dinit::readiness_warnings(&self.artifact_root),
            ServiceManagerKind::Runit => runit::readiness_warnings(),
            ServiceManagerKind::S6 => s6::readiness_warnings(&self.artifact_root, self.live_root()),
        }
    }

    fn live_root(&self) -> &Path {
        self.live_root
            .as_deref()
            .expect("s6 manager should carry a live root")
    }
}
