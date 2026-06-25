//! Installer state snapshots and install checks.
//!
//! Separates read-only state inspection from the execution steps.

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{mpsc::SyncSender, Arc};

use anyhow::Result;

use crate::detect::Detection;
use crate::events::UiMessage;
use crate::model::ActionMode;
use crate::paths::{format_with_home, InstallPaths};

use super::{actions_binaries::resolve_install_binaries_best_effort, log_line};

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
        .all(|artifact| std::fs::symlink_metadata(&artifact.path).is_ok());
    // Enabled state decides whether reinstall can skip `enable --now`
    // Some backends store enablement as installer-owned artifacts instead of manager state
    let mut service_enabled_error = None;
    let service_enabled = if let Some(enabled) = paths.service.enabled_by_artifacts() {
        enabled
    } else {
        match paths.service.is_enabled_command() {
            Some(spec) => match spec.to_command().status() {
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

    InstallState {
        binaries,
        service_artifact_exists,
        service_enabled,
        service_active,
        service_enabled_error,
        service_active_error,
        binary_warning: warning,
    }
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
    for warning in ctx.paths.service.readiness_warnings() {
        log_line(ctx, format!("Warning: {}", warning));
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
#[path = "actions_state/tests.rs"]
mod tests;
