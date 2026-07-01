use std::path::{Path, PathBuf};

use crate::paths::format_with_home;

use super::artifact::ServiceArtifact;
use super::command::CommandSpec;

// Keep the systemd unit name stable for existing installs and migration cleanup
pub const SERVICE_NAME: &str = "unixnotis-daemon.service";

pub fn artifact_label() -> &'static str {
    "systemd unit"
}

pub fn manager_label() -> &'static str {
    "systemd user manager"
}

pub fn primary_artifact_path(artifact_root: &Path) -> PathBuf {
    // systemd uses a single user unit file under the configured user unit directory
    artifact_root.join(SERVICE_NAME)
}

pub fn artifacts(artifact_root: &Path, bin_dir: &Path) -> Vec<ServiceArtifact> {
    vec![ServiceArtifact::file(
        primary_artifact_path(artifact_root),
        render_unit(bin_dir),
    )]
}

pub fn availability_command() -> Option<CommandSpec> {
    Some(
        CommandSpec::new(
            "systemctl --user --no-pager --plain list-units --type=service",
            "systemctl",
            [
                "--user",
                "--no-pager",
                "--plain",
                "list-units",
                "--type=service",
            ],
        )
        .quiet(),
    )
}

pub fn is_enabled_command() -> Option<CommandSpec> {
    Some(CommandSpec::new(
        format!("systemctl --user is-enabled --quiet {SERVICE_NAME}"),
        "systemctl",
        ["--user", "is-enabled", "--quiet", SERVICE_NAME],
    ))
}

pub fn is_active_command() -> Option<CommandSpec> {
    Some(CommandSpec::new(
        format!("systemctl --user is-active --quiet {SERVICE_NAME}"),
        "systemctl",
        ["--user", "is-active", "--quiet", SERVICE_NAME],
    ))
}

pub fn reload_after_artifact_change() -> Option<CommandSpec> {
    Some(CommandSpec::new(
        "systemctl --user daemon-reload",
        "systemctl",
        ["--user", "daemon-reload"],
    ))
}

pub fn enable_now_command() -> Option<CommandSpec> {
    Some(CommandSpec::new(
        format!("systemctl --user enable --now {SERVICE_NAME}"),
        "systemctl",
        ["--user", "enable", "--now", SERVICE_NAME],
    ))
}

pub fn start_command() -> Option<CommandSpec> {
    Some(CommandSpec::new(
        format!("systemctl --user start {SERVICE_NAME}"),
        "systemctl",
        ["--user", "start", SERVICE_NAME],
    ))
}

pub fn disable_now_command() -> Option<CommandSpec> {
    Some(CommandSpec::new(
        format!("systemctl --user disable --now {SERVICE_NAME}"),
        "systemctl",
        ["--user", "disable", "--now", SERVICE_NAME],
    ))
}

pub fn stop_for_reinstall_command() -> Option<CommandSpec> {
    Some(CommandSpec::new(
        format!("systemctl --user --job-mode=replace-irreversibly stop {SERVICE_NAME}"),
        "systemctl",
        [
            "--user",
            "--job-mode=replace-irreversibly",
            "stop",
            SERVICE_NAME,
        ],
    ))
}

pub fn hyprland_startup_commands(import_vars: &[&str]) -> Vec<String> {
    vec![
        format!(
            "dbus-update-activation-environment {}",
            import_vars.join(" ")
        ),
        format!(
            "systemctl --user import-environment {}",
            import_vars.join(" ")
        ),
        format!("systemctl --user --no-block restart {SERVICE_NAME}"),
    ]
}

pub fn environment_sync_commands(
    import_vars: &[(&str, String)],
    dbus_update_available: bool,
) -> Vec<CommandSpec> {
    let mut commands = Vec::new();
    let names = import_vars
        .iter()
        .map(|(name, _value)| *name)
        .collect::<Vec<_>>();
    if dbus_update_available {
        // D-Bus activation and systemd imports solve different environment paths
        commands.push(CommandSpec::new(
            "dbus-update-activation-environment",
            "dbus-update-activation-environment",
            &names,
        ));
    }
    let label = "systemctl --user --no-pager import-environment";
    let mut args = vec!["--user", "--no-pager", "import-environment"];
    // Only caller-filtered session keys are imported, never the whole process env
    args.extend(names);
    commands.push(CommandSpec::new(label, "systemctl", &args));
    commands
}

fn render_unit(bin_dir: &Path) -> String {
    let exec_start = format_exec_start(bin_dir);
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

fn format_exec_start(bin_dir: &Path) -> String {
    let path = bin_dir.join("unixnotis-daemon");
    let rendered = format_with_home(&path);
    if let Some(tail) = rendered.strip_prefix("$HOME") {
        // systemd expands %h itself, while $HOME is not shell-expanded in ExecStart
        format!("%h{tail}")
    } else {
        path.display().to_string()
    }
}
