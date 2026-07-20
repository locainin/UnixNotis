//! Runit envdir publication and supervisor restart flow

use anyhow::Result;
use unixnotis_core::service_manager::ServiceManagerPaths;
use unixnotis_core::CommandSpec;

use super::super::process::{require_success, run};
use super::envdir::write_envdir;

pub(in crate::session_environment) fn sync_runit(manager: &ServiceManagerPaths) -> Result<()> {
    let service = manager.artifact_root.join("unixnotis-daemon");
    write_envdir(&service, &service.join("env"))?;
    let restart = CommandSpec::direct("sv", ["restart".into(), service.as_os_str().to_os_string()]);
    // A successful restart avoids a redundant start request
    if run(&restart)?.success() {
        return Ok(());
    }
    // Fresh installations may exist before the service is supervised
    require_success(&CommandSpec::direct(
        "sv",
        ["start".into(), service.into_os_string()],
    ))
}
