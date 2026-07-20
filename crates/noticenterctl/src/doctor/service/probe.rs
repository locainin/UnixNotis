//! Trusted service-status command construction and backend output parsing

use std::env;
use std::path::Path;
use std::time::Duration;

use unixnotis_core::service_manager::{ServiceManagerKind, ServiceManagerPaths};
use unixnotis_core::CommandSpec;

use crate::debug_logs::journal::daemon_unit_from_env;
use crate::system_tools;

use super::super::report::safe_doctor_text;
use super::super::report::{DoctorCheck, DoctorSeverity};
use super::artifacts::SERVICE_NAME;

const SERVICE_STATUS_TIMEOUT: Duration = Duration::from_secs(3);
const SERVICE_OUTPUT_LIMIT: usize = 4 * 1024;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct SystemdStatus {
    load_state: Option<String>,
    active_state: Option<String>,
    sub_state: Option<String>,
    exec_main_pid: Option<u32>,
}

impl SystemdStatus {
    fn parse(output: &str) -> Self {
        let mut status = Self::default();
        for line in output.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            match key {
                "LoadState" => status.load_state = Some(value.to_string()),
                "ActiveState" => status.active_state = Some(value.to_string()),
                "SubState" => status.sub_state = Some(value.to_string()),
                "ExecMainPID" => status.exec_main_pid = value.parse().ok(),
                _ => {}
            }
        }
        status
    }

    fn is_running_daemon(&self) -> bool {
        self.load_state.as_deref() == Some("loaded")
            && self.active_state.as_deref() == Some("active")
            && self.sub_state.as_deref() == Some("running")
            && self.exec_main_pid.is_some_and(|pid| pid > 0)
    }
}

pub(super) async fn active_candidate(
    kind: ServiceManagerKind,
    paths: &ServiceManagerPaths,
) -> (bool, Option<String>) {
    // Candidate probes and final status checks share one command definition
    let command = match status_command(kind, paths) {
        Ok(command) => command,
        Err(error) => return (false, Some(error)),
    };
    match run_bounded_status(&command).await {
        Ok(output) => {
            let stdout = sanitize_output(&output.stdout);
            (
                status_is_active(kind, output.status.success(), &stdout),
                None,
            )
        }
        Err(error) => (false, Some(error)),
    }
}

pub(super) async fn status_check(
    kind: ServiceManagerKind,
    paths: &ServiceManagerPaths,
) -> DoctorCheck {
    let command = match status_command(kind, paths) {
        Ok(command) => command,
        Err(error) => {
            return DoctorCheck::new(
                "service.status",
                "Service",
                DoctorSeverity::Warning,
                "Service status probe could not be prepared",
            )
            .details(format!("Manager: {}\n{error}", kind.label()))
            .data("manager", kind.label());
        }
    };
    // Probe failures remain warnings so the rest of doctor can explain the install
    let output = match run_bounded_status(&command).await {
        Ok(output) => output,
        Err(error) => {
            return DoctorCheck::new(
                "service.status",
                "Service",
                DoctorSeverity::Warning,
                "Service status probe could not run",
            )
            .details(format!("Manager: {}\n{error}", kind.label()))
            .data("manager", kind.label());
        }
    };
    // Manager output is cleaned before it reaches human or JSON reports
    let stdout = sanitize_output(&output.stdout);
    let active = status_is_active(kind, output.status.success(), &stdout);
    let severity = if active {
        DoctorSeverity::Pass
    } else {
        DoctorSeverity::Warning
    };
    let summary = if active {
        "Service status is active"
    } else {
        "Service status is not an active running daemon"
    };
    let mut check = DoctorCheck::new("service.status", "Service", severity, summary)
        .details(if stdout.is_empty() {
            format!("Manager: {}", kind.label())
        } else {
            format!("Manager: {}\nState: {stdout}", kind.label())
        })
        .data("manager", kind.label())
        .data("command_succeeded", output.status.success())
        .data("active", active);
    // Systemd exposes structured state fields that are useful beyond the summary
    if kind == ServiceManagerKind::Systemd {
        let status = SystemdStatus::parse(&stdout);
        check = check
            .data(
                "load_state",
                status.load_state.unwrap_or_else(|| "unknown".to_string()),
            )
            .data(
                "active_state",
                status.active_state.unwrap_or_else(|| "unknown".to_string()),
            )
            .data(
                "sub_state",
                status.sub_state.unwrap_or_else(|| "unknown".to_string()),
            )
            .data("exec_main_pid", status.exec_main_pid.unwrap_or(0));
    }
    check
}

pub(super) fn status_command(
    kind: ServiceManagerKind,
    paths: &ServiceManagerPaths,
) -> Result<CommandSpec, String> {
    status_command_with_env(kind, paths, |key| env::var(key))
}

pub(super) fn status_command_with_env(
    kind: ServiceManagerKind,
    paths: &ServiceManagerPaths,
    get_var: impl FnOnce(&str) -> Result<String, env::VarError>,
) -> Result<CommandSpec, String> {
    // Only systemd consumes the configurable unit name
    //
    // Keeping this validation inside the systemd branch prevents an invalid
    // systemd-only environment value from disabling another selected backend
    let systemd_unit = if kind == ServiceManagerKind::Systemd {
        daemon_unit_from_env(get_var).map_err(|error| safe_doctor_text(&error.to_string()))?
    } else {
        String::new()
    };
    Ok(status_command_for_unit(kind, paths, &systemd_unit))
}

pub(super) fn status_command_for_unit(
    kind: ServiceManagerKind,
    paths: &ServiceManagerPaths,
    systemd_unit: &str,
) -> CommandSpec {
    // Every backend uses its documented read-only status command
    match kind {
        ServiceManagerKind::Systemd => CommandSpec::direct(
            "systemctl",
            [
                "--user".to_string(),
                "show".to_string(),
                "--property=LoadState".to_string(),
                "--property=ActiveState".to_string(),
                "--property=SubState".to_string(),
                "--property=FragmentPath".to_string(),
                "--property=ExecMainPID".to_string(),
                "--property=ExecMainStatus".to_string(),
                "--no-pager".to_string(),
                "--".to_string(),
                systemd_unit.to_string(),
            ],
        ),
        ServiceManagerKind::Dinit => CommandSpec::direct(
            "dinitctl",
            [
                "--user".to_string(),
                "--quiet".to_string(),
                "is-started".to_string(),
                SERVICE_NAME.to_string(),
            ],
        ),
        ServiceManagerKind::Runit => CommandSpec::direct(
            "sv",
            [
                "status".to_string(),
                paths.artifact_root.join(SERVICE_NAME).display().to_string(),
            ],
        ),
        ServiceManagerKind::S6 => CommandSpec::direct(
            "s6-svstat",
            [
                "-o".to_string(),
                "up".to_string(),
                paths
                    .live_root
                    .as_deref()
                    .unwrap_or_else(|| Path::new(""))
                    .join("servicedirs")
                    .join(SERVICE_NAME)
                    .display()
                    .to_string(),
            ],
        ),
    }
}

async fn run_bounded_status(command: &CommandSpec) -> Result<std::process::Output, String> {
    let program = command
        .program()
        .and_then(Path::to_str)
        .unwrap_or("service manager");
    let process = system_tools::tokio_command_from_spec(command)
        .map_err(|error| safe_doctor_text(&error.to_string()))?
        .output();
    tokio::time::timeout(SERVICE_STATUS_TIMEOUT, process)
        .await
        .map_err(|_elapsed| format!("{program} status probe timed out"))?
        .map_err(|error| safe_doctor_text(&format!("{program} status probe failed: {error}")))
}

pub(super) fn status_is_active(kind: ServiceManagerKind, success: bool, output: &str) -> bool {
    // A failed manager command can never be interpreted as an active daemon
    if !success {
        return false;
    }
    // Output checks follow each manager's stable machine-readable or status format
    match kind {
        ServiceManagerKind::Systemd => SystemdStatus::parse(output).is_running_daemon(),
        ServiceManagerKind::Dinit => true,
        ServiceManagerKind::Runit => output.starts_with("run:") || output.starts_with("run "),
        ServiceManagerKind::S6 => output.trim() == "true",
    }
}

pub(super) fn sanitize_output(bytes: &[u8]) -> String {
    // Bound raw bytes before UTF-8 conversion to cap memory and report size
    let bounded = &bytes[..bytes.len().min(SERVICE_OUTPUT_LIMIT)];
    String::from_utf8_lossy(bounded)
        .lines()
        .map(safe_doctor_text)
        .collect::<Vec<_>>()
        .join("\n")
}
