//! Installer state snapshots and install checks
//!
//! Formats read-only state inspection into progress-log output

use anyhow::{anyhow, Result};

use crate::model::ActionMode;
use crate::paths::format_with_home;
use crate::service_manager::ReadinessIssue;

use super::conflicts::ServiceManagerConflictKind;
use super::install::{check_install_state, reject_conflicting_installation_channel};
use super::{context::ActionContext, log_line, InstallState};

pub fn check_install_state_step(ctx: &mut ActionContext) -> Result<()> {
    // Use cached install state when available to keep the UI consistent with the plan
    let state = ctx
        .install_state
        .clone()
        .unwrap_or_else(|| check_install_state(ctx.paths));

    log_line(ctx, "Install state:");
    if let Some(warning) = state.binary_warning.as_ref() {
        // Surface discovery failures so the install state output stays actionable
        log_line(ctx, format!("Warning: binary discovery failed ({warning})"));
    }
    if state.binaries.is_empty() {
        // Surface metadata/discovery issues early so installer output is actionable
        log_line(ctx, "Warning: no installable binaries discovered");
    }
    for binary in &state.binaries {
        let status = binary.health.label();
        log_line(
            ctx,
            format!(
                "- {}: {} ({})",
                binary.name,
                status,
                format_with_home(&binary.path)
            ),
        );
        if let super::releases::BinaryHealth::Unsafe(detail) = &binary.health {
            log_line(ctx, format!("  inspection failure: {detail}"));
        }
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
        log_line(ctx, format!("- service status check failed: {err}"));
        return Err(anyhow!(
            "cannot establish whether {} is active; refusing to install while service ownership is indeterminate",
            ctx.paths.service.label()
        ));
    }
    if let Some(err) = state.service_enabled_error.as_ref() {
        log_line(ctx, format!("- service enable check failed: {err}"));
    }
    reject_service_manager_conflicts(ctx, &state)?;
    // The source installer must not shadow or combine with package-owned systemd artifacts
    reject_conflicting_installation_channel(ctx)?;
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
    log_install_summary(ctx, &state);
    Ok(())
}

fn log_install_summary(ctx: &mut ActionContext, state: &InstallState) {
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
        // Service inactivity is flagged without blocking installation workflows
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
}

fn reject_service_manager_conflicts(ctx: &mut ActionContext, state: &InstallState) -> Result<()> {
    for warning in &state.service_conflict_warnings {
        // Non-selected backend path issues are diagnostics, not blockers for the selected backend
        log_line(
            ctx,
            format!("Warning: could not inspect another service manager ({warning})"),
        );
    }
    if state.service_conflicts.is_empty() {
        return Ok(());
    }

    // Block before build/copy/write steps so two managers never race to restart the daemon
    for conflict in &state.service_conflicts {
        if conflict.kinds.contains(&ServiceManagerConflictKind::Active) {
            log_line(
                ctx,
                format!(
                    "Error: UnixNotis is active under {}; selected backend is {}",
                    conflict.manager_label,
                    ctx.paths.service.label()
                ),
            );
        }
        if conflict
            .kinds
            .contains(&ServiceManagerConflictKind::Installed)
        {
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
        if conflict
            .kinds
            .contains(&ServiceManagerConflictKind::PartialInstall)
        {
            log_line(
                ctx,
                format!(
                    "Error: incomplete {} remains under {}",
                    conflict.artifact_label, conflict.manager_label
                ),
            );
        }
        if conflict
            .kinds
            .contains(&ServiceManagerConflictKind::UnsafeArtifact)
        {
            log_line(
                ctx,
                format!(
                    "Error: unsafe {} objects remain under {}",
                    conflict.artifact_label, conflict.manager_label
                ),
            );
        }
        if conflict
            .kinds
            .contains(&ServiceManagerConflictKind::Indeterminate)
        {
            log_line(
                ctx,
                format!(
                    "Error: cannot establish whether {} owns or runs UnixNotis",
                    conflict.manager_label
                ),
            );
        }
        for path in &conflict.artifact_paths {
            log_line(ctx, format!("- leftover: {}", format_with_home(path)));
        }
        if let Some(detail) = conflict.detail.as_ref() {
            log_line(ctx, format!("- inspection failure: {detail}"));
        }
    }
    Err(anyhow!(
        "UnixNotis already appears managed by another service manager; uninstall or migrate it before installing with {}",
        ctx.paths.service.label()
    ))
}

#[cfg(test)]
#[path = "tests/state.rs"]
mod tests;
