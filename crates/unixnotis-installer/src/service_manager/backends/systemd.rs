use std::path::{Path, PathBuf};

use super::super::contract::{
    CommandSpec, ServiceArtifact, ServiceManagerAvailability, ServiceManagerAvailabilityOutput,
    ServiceManagerAvailabilityProbe, ServiceProbe, ServiceProbeOutput, ServiceProbeState,
};

// Keep the systemd unit name stable for existing installs and migration cleanup
pub const SERVICE_NAME: &str = "unixnotis-daemon.service";
pub const CONTROL_ACTIVATION_SERVICE: &str = "com.unixnotis.Control.service";

pub const fn artifact_label() -> &'static str {
    "systemd unit"
}

pub const fn manager_label() -> &'static str {
    "systemd user manager"
}

pub fn primary_artifact_path(artifact_root: &Path) -> PathBuf {
    // systemd uses a single user unit file under the configured user unit directory
    artifact_root.join(SERVICE_NAME)
}

pub fn artifacts(artifact_root: &Path, bin_dir: &Path) -> Vec<ServiceArtifact> {
    vec![
        ServiceArtifact::file(primary_artifact_path(artifact_root), render_unit(bin_dir)),
        ServiceArtifact::file(
            control_activation_path(bin_dir),
            render_control_activation(bin_dir),
        ),
    ]
}

pub fn availability_probe() -> ServiceManagerAvailabilityProbe {
    // is-system-running gives one bounded manager state instead of an unbounded unit listing
    let command = CommandSpec::new(
        "systemctl --user is-system-running",
        "systemctl",
        ["--user", "is-system-running"],
    )
    // Transport diagnostics are matched only in the stable C locale
    .env("LC_ALL", "C");
    ServiceManagerAvailabilityProbe::new(command, interpret_availability)
}

fn interpret_availability(
    output: ServiceManagerAvailabilityOutput<'_>,
) -> ServiceManagerAvailability {
    match output.stdout().trim() {
        "initializing" | "starting" | "running" | "degraded" | "maintenance" | "stopping" => {
            ServiceManagerAvailability::Available
        }
        "offline" => ServiceManagerAvailability::Unavailable,
        _ if !output.status_success()
            && output.did_exit()
            && output
                .stderr()
                .trim()
                .starts_with("Failed to connect to bus") =>
        {
            ServiceManagerAvailability::Unavailable
        }
        _ => ServiceManagerAvailability::Indeterminate,
    }
}

pub fn is_enabled_command() -> CommandSpec {
    CommandSpec::new(
        format!("systemctl --user is-enabled --quiet {SERVICE_NAME}"),
        "systemctl",
        ["--user", "is-enabled", "--quiet", SERVICE_NAME],
    )
}

pub fn active_probe() -> ServiceProbe {
    // `show` is systemd's machine-readable state interface
    // A generic nonzero `is-active` result cannot prove the manager was reachable
    let command = CommandSpec::new(
        format!("systemctl --user show LoadState and ActiveState for {SERVICE_NAME}"),
        "systemctl",
        [
            "--user",
            "show",
            "--property=LoadState",
            "--property=ActiveState",
            SERVICE_NAME,
        ],
    );
    ServiceProbe::new(command, interpret_active_state)
}

fn interpret_active_state(output: ServiceProbeOutput<'_>) -> ServiceProbeState {
    if !output.status_success() {
        return ServiceProbeState::Indeterminate;
    }
    let mut load_state = None;
    let mut active_state = None;
    for line in output.stdout().lines() {
        match line.split_once('=') {
            Some(("LoadState", value)) if load_state.replace(value).is_none() => {}
            Some(("ActiveState", value)) if active_state.replace(value).is_none() => {}
            Some(("LoadState" | "ActiveState", _)) | None => {
                return ServiceProbeState::Indeterminate;
            }
            Some((_other, _value)) => {}
        }
    }
    let load_is_known = matches!(
        load_state,
        Some("loaded" | "not-found" | "masked" | "error" | "bad-setting")
    );
    if !load_is_known {
        return ServiceProbeState::Indeterminate;
    }
    match (load_state, active_state) {
        // A missing unit is different from a stopped unit already known to systemd
        (Some("not-found"), Some("inactive")) => ServiceProbeState::Absent,
        (_, Some("active" | "activating" | "deactivating" | "reloading" | "refreshing")) => {
            ServiceProbeState::Active
        }
        (_, Some("inactive" | "failed")) => ServiceProbeState::Inactive,
        (_, Some(_) | None) => ServiceProbeState::Indeterminate,
    }
}

pub fn reload_after_artifact_change() -> CommandSpec {
    CommandSpec::new(
        "systemctl --user daemon-reload",
        "systemctl",
        ["--user", "daemon-reload"],
    )
}

pub fn clear_runtime_mask_command() -> CommandSpec {
    // Explicit installation may clear only temporary state from the current login session
    CommandSpec::new(
        format!("systemctl --user --runtime unmask {SERVICE_NAME}"),
        "systemctl",
        ["--user", "--runtime", "unmask", SERVICE_NAME],
    )
}

pub fn enable_now_command() -> CommandSpec {
    CommandSpec::new(
        format!("systemctl --user enable --now {SERVICE_NAME}"),
        "systemctl",
        ["--user", "enable", "--now", SERVICE_NAME],
    )
}

pub fn start_command() -> CommandSpec {
    CommandSpec::new(
        format!("systemctl --user start {SERVICE_NAME}"),
        "systemctl",
        ["--user", "start", SERVICE_NAME],
    )
}

pub fn disable_now_command() -> CommandSpec {
    CommandSpec::new(
        format!("systemctl --user disable --now {SERVICE_NAME}"),
        "systemctl",
        ["--user", "disable", "--now", SERVICE_NAME],
    )
}

pub fn stop_for_reinstall_command() -> CommandSpec {
    // Stop only this unit during reinstall so systemd never treats the user session as disposable
    CommandSpec::new(
        format!("systemctl --user stop {SERVICE_NAME}"),
        "systemctl",
        ["--user", "stop", SERVICE_NAME],
    )
}

pub fn hyprland_startup_commands(import_vars: &[&str]) -> Vec<String> {
    let allowed = unixnotis_core::service_manager::variables_for_backend(
        unixnotis_core::service_manager::ServiceManagerKind::Systemd,
    );
    let import_vars = import_vars
        .iter()
        .copied()
        .filter(|name| allowed.contains(name))
        .collect::<Vec<_>>();
    vec![
        "systemctl --user unset-environment DBUS_SESSION_BUS_ADDRESS".to_string(),
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
    // Remove a value persisted by older installers before importing safe graphical variables
    let mut commands = vec![CommandSpec::new(
        "systemctl --user unset-environment DBUS_SESSION_BUS_ADDRESS",
        "systemctl",
        ["--user", "unset-environment", "DBUS_SESSION_BUS_ADDRESS"],
    )];
    let allowed = unixnotis_core::service_manager::variables_for_backend(
        unixnotis_core::service_manager::ServiceManagerKind::Systemd,
    );
    let names = import_vars
        .iter()
        .map(|(name, _value)| *name)
        .filter(|name| allowed.contains(name))
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
        // Order after the graphical session without pulling that target into the unit graph
        "After=graphical-session.target".to_string(),
        // Stop this user service when its graphical session is stopped
        "PartOf=graphical-session.target".to_string(),
        String::new(),
        "[Service]".to_string(),
        // Control ownership is published only after notification readiness is verified
        "Type=dbus".to_string(),
        "BusName=com.unixnotis.Control".to_string(),
        format!("ExecStart={exec_start}"),
        "Restart=on-failure".to_string(),
        "RestartSec=1".to_string(),
        "TimeoutStartSec=20".to_string(),
        "TimeoutStopSec=10".to_string(),
        String::new(),
        "[Install]".to_string(),
        "WantedBy=default.target".to_string(),
        String::new(),
    ]
    .join("\n")
}

fn format_exec_start(bin_dir: &Path) -> String {
    let path = bin_dir.join("unixnotis-daemon");
    // The service manager receives one concrete executable with no shell or PATH lookup
    path.display().to_string()
}

fn control_activation_path(bin_dir: &Path) -> PathBuf {
    // Home-local binaries live beside the matching home-local data directory
    let local_root = bin_dir
        .parent()
        .expect("installer binary directory must have a parent");
    local_root
        .join("share")
        .join("dbus-1")
        .join("services")
        .join(CONTROL_ACTIVATION_SERVICE)
}

fn render_control_activation(bin_dir: &Path) -> String {
    let executable = format_exec_start(bin_dir);
    [
        "[D-BUS Service]".to_string(),
        "Name=com.unixnotis.Control".to_string(),
        format!("Exec={executable}"),
        format!("SystemdService={SERVICE_NAME}"),
        String::new(),
    ]
    .join("\n")
}
