//! Session environment synchronization helpers.
//!
//! Keeps user-session environment propagation in one place so install actions can reuse it.

use std::env;
use std::process::Command;

use anyhow::{anyhow, Result};

use unixnotis_core::program_in_path;

use super::{log_line, run_command, ActionContext};

pub(super) const HYPR_IMPORT_VARS: [&str; 7] = [
    "WAYLAND_DISPLAY",
    "XDG_CURRENT_DESKTOP",
    "XDG_SESSION_TYPE",
    "XDG_SESSION_DESKTOP",
    "DISPLAY",
    "XDG_RUNTIME_DIR",
    "PATH",
];
pub(super) const HYPR_REQUIRED_VARS: [&str; 2] = ["WAYLAND_DISPLAY", "XDG_RUNTIME_DIR"];

pub(super) fn sync_user_environment(ctx: &mut ActionContext) -> Result<()> {
    if !program_in_path("systemctl") {
        let message = "systemctl not found; cannot sync user environment";
        log_line(ctx, format!("Error: {}", message));
        return Err(anyhow!(message));
    }

    // Require a minimal Wayland session context before attempting import.
    let missing_required = HYPR_REQUIRED_VARS
        .iter()
        .copied()
        .filter(|var| env::var(var).is_err())
        .collect::<Vec<_>>();
    if !missing_required.is_empty() {
        let message = format!(
            "missing session variables: {}; run from a Wayland session",
            missing_required.join(", ")
        );
        log_line(ctx, format!("Error: {}", message));
        return Err(anyhow!(message));
    }

    // Track whether any environment sync step completed successfully.
    let mut updated = false;
    if program_in_path("dbus-update-activation-environment") {
        // Prefer dbus-update-activation-environment to keep systemd --user in sync with the session.
        let mut command = Command::new("dbus-update-activation-environment");
        command.args(["--systemd", "--all"]);
        if let Err(err) = run_command(
            ctx,
            "dbus-update-activation-environment --systemd --all",
            command,
            None,
        ) {
            log_line(ctx, format!("Warning: {}", err));
        } else {
            updated = true;
        }
    } else {
        log_line(
            ctx,
            "Warning: dbus-update-activation-environment not found; session env may be stale",
        );
    }

    // Import session variables that are commonly missing from systemd --user.
    let vars = HYPR_IMPORT_VARS
        .iter()
        .copied()
        .filter(|var| env::var(var).is_ok())
        .collect::<Vec<_>>();
    if vars.is_empty() {
        let message = "no session environment variables found to import for systemd --user";
        log_line(ctx, format!("Error: {}", message));
        return Err(anyhow!(message));
    } else {
        let mut command = Command::new("systemctl");
        command.args(["--user", "--no-pager", "import-environment"]);
        command.args(&vars);
        if let Err(err) = run_command(
            ctx,
            "systemctl --user --no-pager import-environment",
            command,
            None,
        ) {
            log_line(ctx, format!("Warning: {}", err));
        } else {
            updated = true;
        }
    }

    if !updated {
        let message = "failed to synchronize systemd --user environment";
        log_line(ctx, format!("Error: {}", message));
        return Err(anyhow!(message));
    }

    if updated {
        // Refresh the daemon so newly imported environment variables are picked up.
        let mut command = Command::new("systemctl");
        command.args([
            "--user",
            "--no-pager",
            "restart",
            "unixnotis-daemon.service",
        ]);
        if let Err(err) = run_command(
            ctx,
            "systemctl --user --no-pager restart unixnotis-daemon.service",
            command,
            None,
        ) {
            log_line(ctx, format!("Warning: {}", err));
        }
    }
    Ok(())
}
