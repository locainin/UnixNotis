//! S6-rc envdir publication and live-tree restart flow

use anyhow::{Context, Result};
use unixnotis_core::service_manager::ServiceManagerPaths;
use unixnotis_core::CommandSpec;

use super::super::process::{require_success, run};
use super::envdir::write_envdir;

pub(in crate::session_environment) fn sync_s6(manager: &ServiceManagerPaths) -> Result<()> {
    let service = manager.artifact_root.join("sv").join("unixnotis-daemon");
    write_envdir(&service, &service.join("env"), manager.kind)?;
    let live = manager
        .live_root
        .as_deref()
        .context("s6 live root was not resolved")?;
    // Bringing the compiled service up also refreshes dependency state
    require_success(&CommandSpec::direct(
        "s6-rc",
        [
            "-l".into(),
            live.as_os_str().to_os_string(),
            "-u".into(),
            "change".into(),
            "unixnotis-daemon".into(),
        ],
    ))?;
    let live_service = live.join("servicedirs").join("unixnotis-daemon");
    // The direct service restart is best effort after s6-rc succeeds
    let _ = run(&CommandSpec::direct(
        "s6-svc",
        ["-r".into(), live_service.into_os_string()],
    ))?;
    Ok(())
}
