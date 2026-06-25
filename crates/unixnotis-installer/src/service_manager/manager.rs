use std::path::{Path, PathBuf};

use crate::paths::format_with_home;

use super::artifact::ServiceArtifact;
use super::command::CommandSpec;

// Keep the managed daemon name in one place so command builders and paths stay aligned
pub const UNIXNOTIS_DAEMON_SERVICE: &str = "unixnotis-daemon.service";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServiceManagerKind {
    // User-level systemd is the only supported backend today
    // Future variants must implement the same artifact and command contract before UI wiring
    SystemdUser,
}

impl ServiceManagerKind {
    pub fn label(self) -> &'static str {
        match self {
            // Short label used in status rows and blocking messages
            Self::SystemdUser => "systemd --user",
        }
    }
}

pub struct ServiceManager {
    // Backend choice is stored beside paths so callers do not branch on path layout
    kind: ServiceManagerKind,
    // Stable service name used by lifecycle commands for this backend
    service_name: &'static str,
    // Root directory that receives backend-owned startup artifacts
    artifact_root: PathBuf,
}

impl ServiceManager {
    pub fn systemd_user(artifact_root: PathBuf) -> Self {
        Self {
            kind: ServiceManagerKind::SystemdUser,
            service_name: UNIXNOTIS_DAEMON_SERVICE,
            artifact_root,
        }
    }

    pub fn label(&self) -> &'static str {
        self.kind.label()
    }

    pub fn service_name(&self) -> &'static str {
        // Lifecycle commands should use the backend service name instead of duplicating strings
        self.service_name
    }

    pub fn artifact_label(&self) -> &'static str {
        match self.kind {
            // Artifact means the file or directory the installer owns for startup
            ServiceManagerKind::SystemdUser => "systemd unit",
        }
    }

    pub fn manager_label(&self) -> &'static str {
        match self.kind {
            // Manager label is separate from the artifact label for clearer install logs
            ServiceManagerKind::SystemdUser => "systemd user manager",
        }
    }

    pub fn artifact_root(&self) -> &Path {
        &self.artifact_root
    }

    pub fn primary_artifact_path(&self) -> PathBuf {
        match self.kind {
            // Existing install summaries still display the primary artifact for the active backend
            ServiceManagerKind::SystemdUser => self.artifact_root.join(self.service_name),
        }
    }

    pub fn artifacts(&self, bin_dir: &Path) -> Vec<ServiceArtifact> {
        match self.kind {
            // Rendering lives in the backend so future managers are not forced into unit files
            ServiceManagerKind::SystemdUser => vec![ServiceArtifact::file(
                self.primary_artifact_path(),
                self.render_systemd_unit(bin_dir),
            )],
        }
    }

    pub fn availability_command(&self) -> Option<CommandSpec> {
        match self.kind {
            // list-units proves the user manager is reachable without exposing its environment
            ServiceManagerKind::SystemdUser => Some(
                CommandSpec::new(
                    "systemctl --user --no-pager --plain list-units --type=service",
                    "systemctl",
                    &[
                        "--user",
                        "--no-pager",
                        "--plain",
                        "list-units",
                        "--type=service",
                    ],
                )
                .quiet(),
            ),
        }
    }

    pub fn is_enabled_command(&self) -> Option<CommandSpec> {
        match self.kind {
            // Enabled state decides whether reinstall can skip enable --now
            ServiceManagerKind::SystemdUser => Some(CommandSpec::new(
                format!("systemctl --user is-enabled --quiet {}", self.service_name),
                "systemctl",
                &["--user", "is-enabled", "--quiet", self.service_name],
            )),
        }
    }

    pub fn is_active_command(&self) -> Option<CommandSpec> {
        match self.kind {
            // Active state is used for install summaries and stop recovery checks
            ServiceManagerKind::SystemdUser => Some(CommandSpec::new(
                format!("systemctl --user is-active --quiet {}", self.service_name),
                "systemctl",
                &["--user", "is-active", "--quiet", self.service_name],
            )),
        }
    }

    pub fn reload_after_artifact_change(&self) -> Option<CommandSpec> {
        match self.kind {
            // systemd needs a daemon reload after unit bytes change on disk
            ServiceManagerKind::SystemdUser => Some(CommandSpec::new(
                "systemctl --user daemon-reload",
                "systemctl",
                &["--user", "daemon-reload"],
            )),
        }
    }

    pub fn enable_now_command(&self) -> Option<CommandSpec> {
        match self.kind {
            // First install both enables future logins and starts the daemon now
            ServiceManagerKind::SystemdUser => Some(CommandSpec::new(
                format!("systemctl --user enable --now {}", self.service_name),
                "systemctl",
                &["--user", "enable", "--now", self.service_name],
            )),
        }
    }

    pub fn start_command(&self) -> Option<CommandSpec> {
        match self.kind {
            // Reinstall can start directly when enablement already exists
            ServiceManagerKind::SystemdUser => Some(CommandSpec::new(
                format!("systemctl --user start {}", self.service_name),
                "systemctl",
                &["--user", "start", self.service_name],
            )),
        }
    }

    pub fn disable_now_command(&self) -> Option<CommandSpec> {
        match self.kind {
            // Uninstall must stop the current session and remove login activation
            ServiceManagerKind::SystemdUser => Some(CommandSpec::new(
                format!("systemctl --user disable --now {}", self.service_name),
                "systemctl",
                &["--user", "disable", "--now", self.service_name],
            )),
        }
    }

    pub fn stop_for_reinstall_command(&self) -> Option<CommandSpec> {
        match self.kind {
            // replace-irreversibly avoids a start job canceling the stop during reinstall
            ServiceManagerKind::SystemdUser => Some(CommandSpec::new(
                format!(
                    "systemctl --user --job-mode=replace-irreversibly stop {}",
                    self.service_name
                ),
                "systemctl",
                &[
                    "--user",
                    "--job-mode=replace-irreversibly",
                    "stop",
                    self.service_name,
                ],
            )),
        }
    }

    pub fn hyprland_startup_commands(&self, import_vars: &[&str]) -> Vec<String> {
        match self.kind {
            // Hyprland startup is backend-owned so switching managers cannot leave new systemd lines
            ServiceManagerKind::SystemdUser => vec![
                format!(
                    "dbus-update-activation-environment {}",
                    import_vars.join(" ")
                ),
                format!(
                    "systemctl --user import-environment {}",
                    import_vars.join(" ")
                ),
                format!("systemctl --user --no-block restart {}", self.service_name),
            ],
        }
    }

    pub fn environment_sync_commands(
        &self,
        import_vars: &[&str],
        dbus_update_available: bool,
    ) -> Vec<CommandSpec> {
        match self.kind {
            ServiceManagerKind::SystemdUser => {
                let mut commands = Vec::new();
                if dbus_update_available {
                    // D-Bus activation and service-manager imports solve different env paths
                    commands.push(CommandSpec::new(
                        "dbus-update-activation-environment",
                        "dbus-update-activation-environment",
                        import_vars,
                    ));
                }
                let label = "systemctl --user --no-pager import-environment";
                let mut args = vec!["--user", "--no-pager", "import-environment"];
                // Only caller-filtered session keys are imported, never the whole process env
                args.extend(import_vars);
                commands.push(CommandSpec::new(label, "systemctl", &args));
                commands
            }
        }
    }

    fn render_systemd_unit(&self, bin_dir: &Path) -> String {
        let exec_start = self.format_exec_start(bin_dir);
        [
            "[Unit]".to_string(),
            "Description=UnixNotis Notification Daemon".to_string(),
            "After=graphical-session.target".to_string(),
            "Wants=graphical-session.target".to_string(),
            String::new(),
            "[Service]".to_string(),
            "Type=simple".to_string(),
            format!("ExecStart={exec_start}"),
            "Restart=on-failure".to_string(),
            "RestartSec=1".to_string(),
            String::new(),
            "[Install]".to_string(),
            "WantedBy=default.target".to_string(),
            String::new(),
        ]
        .join("\n")
    }

    fn format_exec_start(&self, bin_dir: &Path) -> String {
        let path = bin_dir.join("unixnotis-daemon");
        let rendered = format_with_home(&path);
        if let Some(tail) = rendered.strip_prefix("$HOME") {
            format!("%h{tail}")
        } else {
            path.display().to_string()
        }
    }
}
