//! Systemd user-manager environment import and restart flow

use anyhow::Result;
use unixnotis_core::CommandSpec;

use crate::system_tools;

use super::super::process::require_success;
use super::super::variables::IMPORT_VARS;

pub(in crate::session_environment) fn sync_systemd() -> Result<()> {
    // D-Bus activation receives the compositor variables when the helper is installed
    if system_tools::trusted_program_path("dbus-update-activation-environment").is_some() {
        require_success(&CommandSpec::direct(
            "dbus-update-activation-environment",
            IMPORT_VARS,
        ))?;
    }
    // The user manager must import the same values before restarting the daemon
    require_success(&CommandSpec::direct(
        "systemctl",
        std::iter::once("--user")
            .chain(std::iter::once("import-environment"))
            .chain(IMPORT_VARS),
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
