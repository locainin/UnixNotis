//! Session environment import for D-Bus activation and the selected service manager

use std::env;

use anyhow::{anyhow, Result};

use unixnotis_core::program_in_path;
use unixnotis_core::service_manager::validate_session_bus_address;

use super::super::{
    install::write_service_artifact, log_line, run_command_without_stdout, ActionContext,
};

const HYPR_REQUIRED_VARS: [&str; 2] = ["WAYLAND_DISPLAY", "XDG_RUNTIME_DIR"];

pub fn sync_user_environment(ctx: &mut ActionContext) -> Result<()> {
    // This step only updates manager environment
    // Service start or restart stays owned by the caller
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
        log_line(ctx, format!("Error: {message}"));
        return Err(anyhow!(message));
    }

    // Track whether at least one sync step finished so partial failures can still succeed
    let mut updated = false;
    let dbus_update_available = program_in_path("dbus-update-activation-environment");
    if !dbus_update_available {
        log_line(
            ctx,
            "Warning: dbus-update-activation-environment not found; session env may be stale",
        );
    }

    // Import only the known session variables that are actually present in this process
    // Missing optional values are left alone so SSH, nested, and unusual sessions still work
    let import_var_names = ctx.paths.service.import_variable_names();
    validate_persisted_bus_address(import_var_names)?;
    let vars = import_var_names
        .iter()
        .copied()
        .filter_map(|var| env::var(var).ok().map(|value| (var, value)))
        .collect::<Vec<_>>();
    if vars.is_empty() {
        let message = "no session environment variables found to import for the service manager";
        log_line(ctx, format!("Error: {message}"));
        return Err(anyhow!(message));
    }

    let env_artifacts = ctx
        .paths
        .service
        .environment_sync_artifacts(import_var_names, &vars);
    for artifact in &env_artifacts {
        // Artifact-based managers persist a small envdir instead of importing into a daemon
        write_service_artifact(ctx, artifact)?;
        updated = true;
    }
    if !env_artifacts.is_empty() {
        log_line(ctx, "Environment synced with service environment files");
    }

    let specs = ctx
        .paths
        .service
        .environment_sync_commands(&vars, dbus_update_available);
    for spec in specs {
        log_line(ctx, format!("Syncing environment with {}", spec.program()));
        // Import commands can echo names or values on stdout on some setups
        let command = match spec.to_command() {
            Ok(command) => command,
            Err(err) => {
                log_line(ctx, format!("Warning: {err}"));
                continue;
            }
        };
        if let Err(err) = run_command_without_stdout(ctx, spec.label(), command, None) {
            log_line(ctx, format!("Warning: {err}"));
            continue;
        }
        log_line(ctx, format!("Environment synced with {}", spec.program()));
        updated = true;
    }

    if !updated {
        // At least one manager must accept the import or the next service start can inherit stale env
        let message = "failed to synchronize service manager environment";
        log_line(ctx, format!("Error: {message}"));
        return Err(anyhow!(message));
    }

    // Service start or restart stays owned by the caller so install avoids double boot
    Ok(())
}

fn validate_persisted_bus_address(import_var_names: &[&str]) -> Result<()> {
    if !import_var_names.contains(&"DBUS_SESSION_BUS_ADDRESS") {
        return Ok(());
    }
    let Ok(address) = env::var("DBUS_SESSION_BUS_ADDRESS") else {
        return Ok(());
    };
    // Direct managers may persist only the stable runtime bus for this uid
    validate_session_bus_address(&address, rustix::process::getuid().as_raw()).map_err(Into::into)
}
