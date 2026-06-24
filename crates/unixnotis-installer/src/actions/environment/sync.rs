//! Session environment import for D-Bus and systemd user manager

use std::env;
use std::process::Command;

use anyhow::{anyhow, Result};

use unixnotis_core::program_in_path;

use super::super::{log_line, run_command_without_stdout, ActionContext};

pub(crate) const HYPR_IMPORT_VARS: [&str; 7] = [
    // Keep this list narrow so debug output and service environments do not inherit full shells
    "WAYLAND_DISPLAY",
    "XDG_CURRENT_DESKTOP",
    "XDG_SESSION_TYPE",
    "XDG_SESSION_DESKTOP",
    "DISPLAY",
    "XDG_RUNTIME_DIR",
    "PATH",
];
const HYPR_REQUIRED_VARS: [&str; 2] = ["WAYLAND_DISPLAY", "XDG_RUNTIME_DIR"];

pub(crate) fn sync_user_environment(ctx: &mut ActionContext) -> Result<()> {
    // This step only updates manager environment
    // Service start or restart stays owned by the caller
    if !program_in_path("systemctl") {
        let message = "systemctl not found; cannot sync user environment";
        log_line(ctx, format!("Error: {}", message));
        return Err(anyhow!(message));
    }

    // Require the minimum Wayland session state before import is attempted
    // Without these two values, a started daemon cannot find the compositor or runtime bus
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

    // Track whether at least one sync step finished so partial failures can still succeed
    let mut updated = false;
    if program_in_path("dbus-update-activation-environment") {
        // Keep D-Bus activation env aligned with the live session
        // Avoid `--systemd` here because systemd import is handled below
        log_line(ctx, "Syncing D-Bus activation environment");
        let mut command = Command::new("dbus-update-activation-environment");
        command.args(HYPR_IMPORT_VARS);
        // Some implementations print every imported variable, which is noisy and can expose local state
        if let Err(err) =
            run_command_without_stdout(ctx, "dbus-update-activation-environment", command, None)
        {
            log_line(ctx, format!("Warning: {}", err));
        } else {
            log_line(ctx, "D-Bus activation environment synced");
            updated = true;
        }
    } else {
        log_line(
            ctx,
            "Warning: dbus-update-activation-environment not found; session env may be stale",
        );
    }

    // Import only the known session variables that are actually present in this process
    // Missing optional values are left alone so SSH, nested, and unusual sessions still work
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
        // Keep the systemd manager env narrow so install does not copy unrelated shell state
        log_line(ctx, "Syncing systemd user environment");
        let mut command = Command::new("systemctl");
        command.args(["--user", "--no-pager", "import-environment"]);
        command.args(&vars);
        // systemctl import-environment may echo names or values on stdout on some setups
        if let Err(err) = run_command_without_stdout(
            ctx,
            "systemctl --user --no-pager import-environment",
            command,
            None,
        ) {
            log_line(ctx, format!("Warning: {}", err));
        } else {
            log_line(ctx, "systemd user environment synced");
            updated = true;
        }
    }

    if !updated {
        // At least one manager must accept the import or the next service start can inherit stale env
        let message = "failed to synchronize systemd --user environment";
        log_line(ctx, format!("Error: {}", message));
        return Err(anyhow!(message));
    }

    // Service start or restart stays owned by the caller so install avoids double boot
    Ok(())
}
