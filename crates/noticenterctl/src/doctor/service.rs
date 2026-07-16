//! Service-manager selection, artifact inspection, and bounded status probes

use std::env;
use std::path::{Path, PathBuf};
use std::time::Duration;

use tokio::process::Command;
use unixnotis_core::service_manager::{
    resolve_service_manager_paths, ServiceManagerKind, ServiceManagerPaths,
};
use unixnotis_core::util::sanitize_inline_display_text;

use crate::cli::DoctorServiceManagerArg;
use crate::debug_logs::journal::daemon_unit_from_env;
use crate::system_tools;

use super::config::{redact_home, redact_home_text};
use super::model::{DoctorCheck, DoctorSeverity};

const SERVICE_NAME: &str = "unixnotis-daemon";
const SERVICE_STATUS_TIMEOUT: Duration = Duration::from_secs(3);
const SERVICE_OUTPUT_LIMIT: usize = 4 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SelectedServiceManager {
    Managed(ServiceManagerKind),
    Manual,
    Unknown,
}

pub(super) struct DoctorServiceResult {
    pub selected: SelectedServiceManager,
    pub checks: Vec<DoctorCheck>,
}

pub(super) async fn inspect_service_manager(
    requested: DoctorServiceManagerArg,
    control_owned: bool,
) -> DoctorServiceResult {
    let mut checks = Vec::new();
    // Selection failure is reported without running commands for an unknown backend
    let selected = match select_service_manager(requested, control_owned).await {
        Ok(selection) => selection,
        Err(check) => {
            checks.push(*check);
            return DoctorServiceResult {
                selected: SelectedServiceManager::Unknown,
                checks,
            };
        }
    };

    // Managed backends always report both artifact and live status checks
    match selected {
        SelectedServiceManager::Managed(kind) => match resolve_service_manager_paths(kind) {
            Ok(paths) => {
                checks.push(artifact_check(&paths));
                checks.push(status_check(kind, &paths).await);
            }
            Err(error) => checks.push(
                DoctorCheck::new(
                    "service.paths",
                    "Service",
                    DoctorSeverity::Error,
                    "Service-manager paths could not be resolved",
                )
                .details(error.to_string()),
            ),
        },
        SelectedServiceManager::Manual => checks.push(
            DoctorCheck::new(
                "service.status",
                "Service",
                if control_owned {
                    DoctorSeverity::Pass
                } else {
                    DoctorSeverity::Warning
                },
                if control_owned {
                    "Manual launch is reachable through D-Bus"
                } else {
                    "Manual launch is not reachable through D-Bus"
                },
            )
            .details("Manager: manual"),
        ),
        SelectedServiceManager::Unknown => checks.push(
            DoctorCheck::new(
                "service.status",
                "Service",
                DoctorSeverity::Note,
                "No service manager was selected",
            )
            .details("Manager: unknown")
            .hint("Pass --service-manager when UnixNotis uses a supported manager"),
        ),
    }

    DoctorServiceResult { selected, checks }
}

async fn select_service_manager(
    requested: DoctorServiceManagerArg,
    control_owned: bool,
) -> Result<SelectedServiceManager, Box<DoctorCheck>> {
    // Command-line selection has the highest priority and avoids auto probes
    let explicit = match requested {
        DoctorServiceManagerArg::Systemd => Some(ServiceManagerKind::Systemd),
        DoctorServiceManagerArg::Dinit => Some(ServiceManagerKind::Dinit),
        DoctorServiceManagerArg::Runit => Some(ServiceManagerKind::Runit),
        DoctorServiceManagerArg::S6 => Some(ServiceManagerKind::S6),
        DoctorServiceManagerArg::Manual => return Ok(SelectedServiceManager::Manual),
        DoctorServiceManagerArg::Auto => None,
    };
    if let Some(kind) = explicit {
        return Ok(SelectedServiceManager::Managed(kind));
    }

    // Installer-provided environment selection is the next trusted signal
    if let Some(selection) = manager_from_environment(env::var_os("UNIXNOTIS_SERVICE_MANAGER"))? {
        return Ok(selection);
    }

    // Auto mode records every plausible backend instead of choosing the first match
    let mut candidates = Vec::new();
    let mut path_errors = Vec::new();
    for kind in ServiceManagerKind::all() {
        // Every backend is inspected so ambiguity can be reported instead of hidden
        match resolve_service_manager_paths(kind) {
            Ok(paths) => {
                // Installed artifacts are strongest, while an active probe recovers manual moves
                if candidate_is_present(
                    primary_artifact(&paths).exists(),
                    active_candidate(kind, &paths).await,
                ) {
                    candidates.push(kind);
                }
            }
            Err(error) => path_errors.push(format!("{}: {error}", kind.label())),
        }
    }
    select_detected_manager(&candidates, &path_errors, control_owned)
}

fn manager_from_environment(
    raw: Option<std::ffi::OsString>,
) -> Result<Option<SelectedServiceManager>, Box<DoctorCheck>> {
    // Empty values mean no override while malformed values remain objective errors
    let Some(raw) = raw.filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let raw = raw.to_string_lossy();
    // Lossy conversion is safe here because unsupported values still fail explicitly
    ServiceManagerKind::parse_explicit(&raw)
        .map(SelectedServiceManager::Managed)
        .map(Some)
        .map_err(|error| {
            Box::new(
                DoctorCheck::new(
                    "service.selection",
                    "Service",
                    DoctorSeverity::Error,
                    "UNIXNOTIS_SERVICE_MANAGER is invalid",
                )
                .details(error.to_string()),
            )
        })
}

const fn candidate_is_present(artifact_exists: bool, active_probe: bool) -> bool {
    artifact_exists || active_probe
}

fn select_detected_manager(
    candidates: &[ServiceManagerKind],
    path_errors: &[String],
    control_owned: bool,
) -> Result<SelectedServiceManager, Box<DoctorCheck>> {
    // One candidate is safe while zero and multiple candidates need context
    match candidates {
        [kind] => Ok(SelectedServiceManager::Managed(*kind)),
        [] if control_owned => Ok(SelectedServiceManager::Manual),
        [] if path_errors.is_empty() => Ok(SelectedServiceManager::Unknown),
        [] => Err(Box::new(
            DoctorCheck::new(
                "service.selection",
                "Service",
                DoctorSeverity::Warning,
                "Service-manager artifacts could not be inspected completely",
            )
            .details(path_errors.join("\n"))
            .hint("Pass --service-manager to select the active backend"),
        )),
        many => Err(Box::new(
            DoctorCheck::new(
                "service.selection",
                "Service",
                DoctorSeverity::Warning,
                "Multiple UnixNotis service-manager artifacts were found",
            )
            .details(
                // Candidate labels are fixed strings and safe for issue attachments
                many.iter()
                    .map(|manager| manager.label())
                    .collect::<Vec<_>>()
                    .join(", "),
            )
            .hint("Pass --service-manager to select the active backend"),
        )),
    }
}

async fn active_candidate(kind: ServiceManagerKind, paths: &ServiceManagerPaths) -> bool {
    // Probe failures mean no active evidence and never abort the remaining managers
    let (program, args) = status_command(kind, paths);
    let Ok(output) = run_bounded_status(program, &args).await else {
        return false;
    };
    let stdout = sanitize_output(&output.stdout);
    status_is_active(kind, output.status.success(), &stdout)
}

fn primary_artifact(paths: &ServiceManagerPaths) -> PathBuf {
    // These paths mirror the installer's actual service artifacts
    match paths.kind {
        ServiceManagerKind::Systemd => paths.artifact_root.join("unixnotis-daemon.service"),
        ServiceManagerKind::Dinit => paths.artifact_root.join(SERVICE_NAME),
        ServiceManagerKind::Runit => paths.artifact_root.join(SERVICE_NAME).join("run"),
        ServiceManagerKind::S6 => paths
            .artifact_root
            .join("sv")
            .join(SERVICE_NAME)
            .join("run"),
    }
}

fn artifact_check(paths: &ServiceManagerPaths) -> DoctorCheck {
    // A missing artifact is suspicious but a manual launch can still be healthy
    let artifact = primary_artifact(paths);
    if artifact.is_file() {
        DoctorCheck::new(
            "service.artifact",
            "Service artifact",
            DoctorSeverity::Pass,
            "Installed service artifact was found",
        )
        .details(format!(
            "Manager: {}\nArtifact: {}",
            paths.kind.label(),
            redact_home(&artifact)
        ))
    } else {
        DoctorCheck::new(
            "service.artifact",
            "Service artifact",
            DoctorSeverity::Warning,
            "Installed service artifact was not found",
        )
        .details(format!(
            "Manager: {}\nExpected: {}",
            paths.kind.label(),
            redact_home(&artifact)
        ))
    }
}

async fn status_check(kind: ServiceManagerKind, paths: &ServiceManagerPaths) -> DoctorCheck {
    // Status failures stay warnings because D-Bus readiness is checked separately
    let (program, args) = status_command(kind, paths);
    let output = match run_bounded_status(program, &args).await {
        Ok(output) => output,
        Err(error) => {
            return DoctorCheck::new(
                "service.status",
                "Service",
                DoctorSeverity::Warning,
                "Service status probe could not run",
            )
            .details(format!("Manager: {}\n{error}", kind.label()));
        }
    };
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
        "Service status is not active"
    };
    let mut details = format!("Manager: {}", kind.label());
    if !stdout.is_empty() {
        // State output follows the manager line without flattening systemctl properties
        details.push_str("\nState: ");
        details.push_str(&stdout);
    }
    DoctorCheck::new("service.status", "Service", severity, summary).details(details)
}

fn status_command(
    kind: ServiceManagerKind,
    paths: &ServiceManagerPaths,
) -> (&'static str, Vec<String>) {
    let systemd_unit = daemon_unit_from_env(|key| env::var(key));
    status_command_for_unit(kind, paths, &systemd_unit)
}

fn status_command_for_unit(
    kind: ServiceManagerKind,
    paths: &ServiceManagerPaths,
    systemd_unit: &str,
) -> (&'static str, Vec<String>) {
    // Only commands installed and documented by each backend are used here
    match kind {
        ServiceManagerKind::Systemd => (
            "systemctl",
            vec![
                "--user".to_string(),
                "show".to_string(),
                // Status and journal collection must inspect the same configured unit
                systemd_unit.to_string(),
                "--property=LoadState".to_string(),
                "--property=ActiveState".to_string(),
                "--property=SubState".to_string(),
                "--property=FragmentPath".to_string(),
                "--property=ExecMainPID".to_string(),
                "--property=ExecMainStatus".to_string(),
                "--no-pager".to_string(),
            ],
        ),
        ServiceManagerKind::Dinit => (
            "dinitctl",
            vec![
                "--user".to_string(),
                "--quiet".to_string(),
                "is-started".to_string(),
                SERVICE_NAME.to_string(),
            ],
        ),
        ServiceManagerKind::Runit => (
            "sv",
            vec![
                "status".to_string(),
                // Runit status receives the exact installed service directory
                paths.artifact_root.join(SERVICE_NAME).display().to_string(),
            ],
        ),
        ServiceManagerKind::S6 => (
            "s6-svstat",
            vec![
                "-o".to_string(),
                "up".to_string(),
                paths
                    .live_root
                    .as_deref()
                    // Missing live roots remain an empty contained path and fail the probe
                    .unwrap_or_else(|| Path::new(""))
                    .join("servicedirs")
                    .join(SERVICE_NAME)
                    .display()
                    .to_string(),
            ],
        ),
    }
}

async fn run_bounded_status(
    program: &str,
    args: &[String],
) -> Result<std::process::Output, String> {
    // Trusted fixed directories avoid user-controlled PATH command replacement
    let path = system_tools::trusted_program_path(program)
        .ok_or_else(|| format!("{program} was not found in trusted system directories"))?;
    let command = Command::new(path).args(args).output();
    tokio::time::timeout(SERVICE_STATUS_TIMEOUT, command)
        .await
        .map_err(|_elapsed| format!("{program} status probe timed out"))?
        .map_err(|error| format!("{program} status probe failed: {error}"))
}

fn status_is_active(kind: ServiceManagerKind, success: bool, output: &str) -> bool {
    // A successful process exit is required before backend output is trusted
    if !success {
        return false;
    }
    match kind {
        // Each parser accepts only the manager's documented active representation
        ServiceManagerKind::Systemd => output.lines().any(|line| line == "ActiveState=active"),
        ServiceManagerKind::Dinit => true,
        ServiceManagerKind::Runit => output.starts_with("run:") || output.starts_with("run "),
        ServiceManagerKind::S6 => output.trim() == "true",
    }
}

fn sanitize_output(bytes: &[u8]) -> String {
    // Cap bytes before UTF-8 replacement so hostile output cannot grow without bound
    let bounded = &bytes[..bytes.len().min(SERVICE_OUTPUT_LIMIT)];
    // Preserve record boundaries because systemctl reports one property per line
    let sanitized = String::from_utf8_lossy(bounded)
        .lines()
        .map(sanitize_inline_display_text)
        .collect::<Vec<_>>()
        .join("\n");
    // Home redaction runs last so replacement cannot reintroduce control characters
    redact_home_text(&sanitized)
}

#[cfg(test)]
#[path = "tests/service.rs"]
mod tests;
