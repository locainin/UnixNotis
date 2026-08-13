//! Systemd user-manager environment import and restart flow

use anyhow::Result;
use unixnotis_core::service_manager::ServiceManagerKind;
use unixnotis_core::CommandSpec;

use crate::system_tools;

use super::super::process::require_success;
use super::super::variables::import_variables;

pub(in crate::session_environment) fn sync_systemd() -> Result<()> {
    let import_variables = import_variables(ServiceManagerKind::Systemd);
    // Older installer releases may have persisted a transient nested-session address
    require_success(&CommandSpec::direct(
        "systemctl",
        ["--user", "unset-environment", "DBUS_SESSION_BUS_ADDRESS"],
    ))?;
    // D-Bus activation receives the compositor variables when the helper is installed
    if system_tools::trusted_program_path("dbus-update-activation-environment").is_some() {
        require_success(&CommandSpec::direct(
            "dbus-update-activation-environment",
            import_variables,
        ))?;
    }
    // The user manager must import the same values before restarting the daemon
    require_success(&CommandSpec::direct(
        "systemctl",
        std::iter::once("--user")
            .chain(std::iter::once("import-environment"))
            .chain(import_variables.iter().copied()),
    ))?;
    require_success(&CommandSpec::direct(
        "systemctl",
        [
            "--user",
            "--no-block",
            "restart",
            "unixnotis-daemon.service",
        ],
    ))
}
