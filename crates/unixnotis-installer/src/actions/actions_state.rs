//! Installer state snapshots and install checks.
//!
//! Separates read-only state inspection from the execution steps.

use std::path::PathBuf;
use std::process::Command;
use std::sync::mpsc::SyncSender;

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
    unit_exists: bool,
    unit_active: bool,
    unit_active_error: Option<String>,
    binary_warning: Option<String>,
}

impl InstallState {
    pub fn is_installed(&self) -> bool {
        // Treat installed as binaries + unit present; runtime status tracked separately.
        !self.binaries.is_empty()
            && self.binaries.iter().all(|binary| binary.exists)
            && self.unit_exists
    }

    pub fn is_fully_installed(&self) -> bool {
        self.is_installed() && self.unit_active
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

    let unit_exists = paths.unit_path.exists();
    let mut unit_active_error = None;
    let unit_active = match Command::new("systemctl")
        .args(["--user", "is-active", "--quiet", "unixnotis-daemon.service"])
        .status()
    {
        Ok(status) => status.success(),
        Err(err) => {
            unit_active_error = Some(err.to_string());
            false
        }
    };

    InstallState {
        binaries,
        unit_exists,
        unit_active,
        unit_active_error,
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

    let unit_status = if state.unit_exists {
        "present"
    } else {
        "missing"
    };
    log_line(
        ctx,
        format!(
            "- systemd unit: {} ({})",
            unit_status,
            format_with_home(&ctx.paths.unit_path)
        ),
    );
    if let Some(err) = state.unit_active_error.as_ref() {
        log_line(ctx, format!("- systemd status check failed: {}", err));
    }
    log_line(
        ctx,
        format!(
            "- service active: {}",
            if state.unit_active { "yes" } else { "no" }
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
            "Warning: installation is present but unixnotis-daemon.service is inactive",
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
