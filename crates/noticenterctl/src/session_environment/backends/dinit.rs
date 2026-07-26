//! Dinit user-manager environment import and start flow

use anyhow::Result;
use unixnotis_core::service_manager::ServiceManagerKind;
use unixnotis_core::CommandSpec;

use super::super::process::{require_success, run};
use super::super::variables::import_variables;

pub(in crate::session_environment) fn sync_dinit() -> Result<()> {
    let import_variables = import_variables(ServiceManagerKind::Dinit);
    // Dinit imports named values directly from the current process environment
    require_success(&CommandSpec::direct(
        "dinitctl",
        std::iter::once("--user")
            .chain(std::iter::once("setenv"))
            .chain(import_variables.iter().copied()),
    ))?;
    let restart = CommandSpec::direct(
        "dinitctl",
        [
            "--user",
            "restart",
            "--ignore-unstarted",
            "unixnotis-daemon",
        ],
    );
    // An inactive service cannot restart, so a normal start follows every attempt
    let _ = run(&restart)?;
    require_success(&CommandSpec::direct(
        "dinitctl",
        ["--user", "start", "unixnotis-daemon"],
    ))
}
