//! Installer state snapshots and install checks.
//!
//! Separates read-only state inspection from the execution steps.

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{mpsc::SyncSender, Arc};

use anyhow::{anyhow, Result};

use crate::detect::Detection;
use crate::events::UiMessage;
use crate::model::ActionMode;
use crate::paths::{format_with_home, InstallPaths};
use crate::service_manager::ReadinessIssue;

use super::{binaries::resolve_install_binaries_best_effort, log_line};

pub struct ActionContext<'a> {
    pub detection: &'a Detection,
    pub paths: &'a InstallPaths,
    pub install_state: Option<InstallState>,
    pub log_tx: SyncSender<UiMessage>,
    pub action_mode: ActionMode,
    pub restore_backup: Option<PathBuf>,
    // Tracks whether install changed the service artifact so later steps can skip reloads
    // A skipped reload keeps reinstall cheap when the on-disk artifact already matched
    pub service_reload_required: Arc<AtomicBool>,
}

#[derive(Clone)]
struct BinaryState {
    name: String,
    path: PathBuf,
    exists: bool,
}

#[derive(Clone)]
pub struct InstallState {
    binaries: Vec<BinaryState>,
    service_artifact_exists: bool,
    service_enabled: bool,
    service_active: bool,
    service_enabled_error: Option<String>,
    service_active_error: Option<String>,
    binary_warning: Option<String>,
    service_conflicts: Vec<ServiceManagerConflict>,
    service_conflict_warnings: Vec<String>,
}

#[derive(Clone)]
struct ServiceManagerConflict {
    // User-facing manager name for the backend that appears to own UnixNotis already
    manager_label: &'static str,
    // Artifact wording stays backend-specific so errors are clear for s6/runit directories
    artifact_label: &'static str,
    // Primary artifact path gives the user one concrete place to inspect
    artifact_path: PathBuf,
    // Installed means every steady artifact for the other backend matches the safe shape
    installed: bool,
    // Active means the other backend's native runtime probe says its daemon is running
    active: bool,
}

impl InstallState {
    pub fn is_installed(&self) -> bool {
        // Treat installed as binaries plus the service artifact; runtime status is separate
        !self.binaries.is_empty()
            && self.binaries.iter().all(|binary| binary.exists)
            && self.service_artifact_exists
    }

    pub fn is_fully_installed(&self) -> bool {
        self.is_installed() && self.service_active
    }

    pub fn service_enabled(&self) -> bool {
        self.service_enabled
    }
}

pub fn check_install_state(paths: &InstallPaths) -> InstallState {
    // Keep install state aligned with installer binary discovery.
    // Best-effort resolution keeps install state usable even if workspace metadata is broken.
    let (binaries, warning) = resolve_install_binaries_best_effort(paths);
    let binaries = binaries
        .into_iter()
        .map(|name| {
            let path = paths.bin_dir.join(&name);
            BinaryState {
                name,
                exists: path.exists(),
                path,
            }
        })
        .collect::<Vec<_>>();

    let service_artifact_exists = paths
        .service
        .artifacts(&paths.bin_dir)
        .iter()
        // Artifact presence must prove the expected file, directory marker, or link target
        .all(crate::service_manager::ServiceArtifact::is_present_safely);
    // Enabled state decides whether reinstall can skip `enable --now`
    // Some backends store enablement as installer-owned artifacts instead of manager state
    let mut service_enabled_error = None;
    let service_enabled = if let Some(enabled) = paths.service.enabled_by_artifacts() {
        // Artifact-backed managers prove enablement through installer-owned filesystem state
        enabled
    } else {
        match paths.service.is_enabled_command() {
            Some(spec) => match spec.to_command().status() {
                // Command-backed managers still use the native manager status probe
                Ok(status) => status.success(),
                Err(err) => {
                    service_enabled_error = Some(err.to_string());
                    false
                }
            },
            None => {
                // This should only apply to future backends that have no state probe yet
                service_enabled_error =
                    Some("service manager has no enabled-state command".to_string());
                false
            }
        }
    };
    // Active state still matters for the install summary shown in the UI
    let mut service_active_error = None;
    let service_active = match paths.service.active_probe() {
        Some(probe) => match probe.evaluate() {
            // Active probes can be plain exit status or stdout parsing, depending on backend
            Ok(active) => active,
            Err(err) => {
                service_active_error = Some(err.to_string());
                false
            }
        },
        None => {
            // Backends without active state still allow install, but cannot claim a running service
            service_active_error = Some("service manager has no active-state command".to_string());
            false
        }
    };

    let (service_conflicts, service_conflict_warnings) =
        detect_service_manager_conflict_state(paths);

    InstallState {
        binaries,
        service_artifact_exists,
        service_enabled,
        service_active,
        service_enabled_error,
        service_active_error,
        binary_warning: warning,
        service_conflicts,
        service_conflict_warnings,
    }
}

fn detect_service_manager_conflict_state(
    paths: &InstallPaths,
) -> (Vec<ServiceManagerConflict>, Vec<String>) {
    let mut conflicts = Vec::new();
    let mut warnings = Vec::new();

    // Selected-backend reinstall is valid, but sibling backends must not keep owning the daemon
    for manager in paths.alternate_service_managers() {
        let manager = match manager {
            Ok(manager) => manager,
            Err(err) => {
                // A broken non-selected backend path should be visible but should not block install
                warnings.push(err.to_string());
                continue;
            }
        };

        // Artifact ownership uses the same safe shape checks as selected-backend state
        let artifacts = manager.artifacts(&paths.bin_dir);
        let installed = !artifacts.is_empty()
            && artifacts
                .iter()
                .all(crate::service_manager::ServiceArtifact::is_present_safely);
        // Active probes are best-effort because missing tools should not become false conflicts
        let active = manager
            .active_probe()
            .and_then(|probe| probe.evaluate().ok())
            .unwrap_or(false);

        // Only real evidence should block install; probe errors are treated as not active
        if installed || active {
            conflicts.push(ServiceManagerConflict {
                manager_label: manager.label(),
                artifact_label: manager.artifact_label(),
                artifact_path: manager.primary_artifact_path(),
                installed,
                active,
            });
        }
    }

    (conflicts, warnings)
}

pub fn check_install_state_step(ctx: &mut ActionContext) -> Result<()> {
    // Use cached install state when available to keep the UI consistent with the plan.
    let state = ctx
        .install_state
        .clone()
        .unwrap_or_else(|| check_install_state(ctx.paths));

    log_line(ctx, "Install state:");
    if let Some(warning) = state.binary_warning.as_ref() {
        // Surface discovery failures so the install state output stays actionable.
        log_line(ctx, format!("Warning: binary discovery failed ({warning})"));
    }
    if state.binaries.is_empty() {
        // Surface metadata/discovery issues early so installer output is actionable.
        log_line(ctx, "Warning: no installable binaries discovered");
    }
    for binary in &state.binaries {
        let status = if binary.exists { "present" } else { "missing" };
        log_line(
            ctx,
            format!(
                "- {}: {} ({})",
                binary.name,
                status,
                format_with_home(&binary.path)
            ),
        );
    }

    let service_artifact_status = if state.service_artifact_exists {
        "present"
    } else {
        "missing"
    };
    log_line(
        ctx,
        format!(
            "- {}: {} ({})",
            ctx.paths.service.artifact_label(),
            service_artifact_status,
            format_with_home(&ctx.paths.service.primary_artifact_path())
        ),
    );
    if let Some(err) = state.service_active_error.as_ref() {
        log_line(ctx, format!("- service status check failed: {}", err));
    }
    if let Some(err) = state.service_enabled_error.as_ref() {
        log_line(ctx, format!("- service enable check failed: {}", err));
    }
    for warning in &state.service_conflict_warnings {
        // Non-selected backend path issues are diagnostics, not blockers for the selected backend
        log_line(
            ctx,
            format!("Warning: could not inspect another service manager ({warning})"),
        );
    }
    if !state.service_conflicts.is_empty() {
        // Block before build/copy/write steps so two managers never race to restart the daemon
        for conflict in &state.service_conflicts {
            if conflict.active {
                log_line(
                    ctx,
                    format!(
                        "Error: UnixNotis is active under {}; selected backend is {}",
                        conflict.manager_label,
                        ctx.paths.service.label()
                    ),
                );
            }
            if conflict.installed {
                log_line(
                    ctx,
                    format!(
                        "Error: {} already exists under {} at {}",
                        conflict.artifact_label,
                        conflict.manager_label,
                        format_with_home(&conflict.artifact_path)
                    ),
                );
            }
        }
        return Err(anyhow!(
            "UnixNotis already appears managed by another service manager; uninstall or migrate it before installing with {}",
            ctx.paths.service.label()
        ));
    }
    let mut readiness_errors = Vec::new();
    for issue in ctx.paths.service.readiness_issues() {
        match issue {
            // Warnings are visible in logs but do not stop already-safe install flows
            ReadinessIssue::Warning(message) => log_line(ctx, format!("Warning: {message}")),
            ReadinessIssue::Error(message) => {
                // Errors mean the backend would fail after writing files, so stop here
                log_line(ctx, format!("Error: {message}"));
                readiness_errors.push(message);
            }
        }
    }
    if !readiness_errors.is_empty() {
        // Return one combined error so the progress screen has a concise failure summary
        return Err(anyhow!(
            "{} is not ready: {}",
            ctx.paths.service.label(),
            readiness_errors.join("; ")
        ));
    }
    log_line(
        ctx,
        format!(
            "- service enabled: {}",
            if state.service_enabled { "yes" } else { "no" }
        ),
    );
    log_line(
        ctx,
        format!(
            "- service active: {}",
            if state.service_active { "yes" } else { "no" }
        ),
    );

    if state.is_fully_installed() {
        if matches!(ctx.action_mode, ActionMode::Install) {
            log_line(
                ctx,
                "Already installed. Reinstall will overwrite existing files.",
            );
        } else {
            log_line(ctx, "Already installed.");
        }
    } else if state.is_installed() {
        // Service inactivity is flagged without blocking installation workflows.
        log_line(
            ctx,
            format!(
                "Warning: installation is present but {} is inactive",
                ctx.paths.service.service_name()
            ),
        );
        log_line(
            ctx,
            "Hint: logout/login should refresh session environment and restart the service",
        );
    } else {
        log_line(ctx, "Install will continue and update missing items.");
    }

    Ok(())
}

#[cfg(test)]
#[path = "tests/state.rs"]
mod tests;
