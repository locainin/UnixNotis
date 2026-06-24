//! Service-manager metadata for installer-owned daemon startup.

use std::path::{Path, PathBuf};
use std::process::Command;

// Keep the managed daemon name in one place so command builders and paths stay aligned
pub const UNIXNOTIS_DAEMON_SERVICE: &str = "unixnotis-daemon.service";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServiceManagerKind {
    // User-level systemd is the only supported backend today
    // Future variants should carry the same install/start/stop contract before wiring UI flows
    SystemdUser,
}

impl ServiceManagerKind {
    pub fn label(self) -> &'static str {
        match self {
            // Short label used in status rows and blocking messages
            Self::SystemdUser => "systemd --user",
        }
    }

    pub fn artifact_label(self) -> &'static str {
        match self {
            // Artifact means the file or directory the installer owns for startup
            Self::SystemdUser => "systemd unit",
        }
    }

    pub fn manager_label(self) -> &'static str {
        match self {
            // Manager label is separate from the artifact label for clearer install logs
            Self::SystemdUser => "systemd user manager",
        }
    }

    pub fn availability_check(self) -> Command {
        match self {
            Self::SystemdUser => {
                // show-environment proves the user manager is reachable without changing state
                let mut command = Command::new("systemctl");
                command.args(["--user", "show-environment"]);
                command
            }
        }
    }
}

pub struct ServiceManagerPaths {
    // Backend choice is stored beside paths so callers do not branch on path layout
    kind: ServiceManagerKind,
    // Stable service name used by lifecycle commands for this backend
    service_name: &'static str,
    // Directory that receives the backend-owned startup artifact
    artifact_dir: PathBuf,
    // Full path to the exact installer-managed startup artifact
    artifact_path: PathBuf,
}

impl ServiceManagerPaths {
    pub fn systemd_user(artifact_dir: PathBuf) -> Self {
        // systemd stores one user unit file under the XDG config service directory
        let artifact_path = artifact_dir.join(UNIXNOTIS_DAEMON_SERVICE);
        Self {
            kind: ServiceManagerKind::SystemdUser,
            service_name: UNIXNOTIS_DAEMON_SERVICE,
            artifact_dir,
            artifact_path,
        }
    }

    pub fn service_name(&self) -> &'static str {
        // Lifecycle commands should use the backend service name instead of duplicating strings
        self.service_name
    }

    pub fn artifact_dir(&self) -> &Path {
        // Install checks probe this directory before writing any startup files
        &self.artifact_dir
    }

    pub fn artifact_path(&self) -> &Path {
        // Writers compare and replace this exact file atomically when possible
        &self.artifact_path
    }

    pub fn artifact_label(&self) -> &'static str {
        // Log output can name the artifact without knowing which backend owns it
        self.kind.artifact_label()
    }

    pub fn manager_label(&self) -> &'static str {
        // Reload messages should name the manager, not the file format
        self.kind.manager_label()
    }

    pub fn is_enabled_command(&self) -> Command {
        match self.kind {
            ServiceManagerKind::SystemdUser => {
                // Quiet status checks keep install-state reads side-effect free
                let mut command = Command::new("systemctl");
                command.args(["--user", "is-enabled", "--quiet", self.service_name]);
                command
            }
        }
    }

    pub fn is_active_command(&self) -> Command {
        match self.kind {
            ServiceManagerKind::SystemdUser => {
                // Active state is used for summaries and reinstall decisions only
                let mut command = Command::new("systemctl");
                command.args(["--user", "is-active", "--quiet", self.service_name]);
                command
            }
        }
    }

    pub fn daemon_reload_command(&self) -> (String, Command) {
        match self.kind {
            ServiceManagerKind::SystemdUser => {
                // systemd needs a reload after unit bytes change on disk
                let mut command = Command::new("systemctl");
                command.args(["--user", "daemon-reload"]);
                ("systemctl --user daemon-reload".to_string(), command)
            }
        }
    }

    pub fn enable_now_command(&self) -> (String, Command) {
        match self.kind {
            ServiceManagerKind::SystemdUser => {
                // First install both enables future logins and starts the daemon now
                let mut command = Command::new("systemctl");
                command.args(["--user", "enable", "--now", self.service_name]);
                (
                    format!("systemctl --user enable --now {}", self.service_name),
                    command,
                )
            }
        }
    }

    pub fn start_command(&self) -> (String, Command) {
        match self.kind {
            ServiceManagerKind::SystemdUser => {
                // Reinstall can start directly when enablement already exists
                let mut command = Command::new("systemctl");
                command.args(["--user", "start", self.service_name]);
                (
                    format!("systemctl --user start {}", self.service_name),
                    command,
                )
            }
        }
    }

    pub fn disable_now_command(&self) -> (String, Command) {
        match self.kind {
            ServiceManagerKind::SystemdUser => {
                // Uninstall must stop the current session and remove login activation
                let mut command = Command::new("systemctl");
                command.args(["--user", "disable", "--now", self.service_name]);
                (
                    format!("systemctl --user disable --now {}", self.service_name),
                    command,
                )
            }
        }
    }
}
