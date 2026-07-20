use std::fs;
use std::os::unix::fs::PermissionsExt;

use unixnotis_core::service_manager::{ServiceManagerKind, ServiceManagerPaths};

use super::super::super::backends::sync_runit;
use super::super::support::TempToolDir;
use crate::system_tools::routing::use_fake_tool_bin;

#[test]
fn runit_sync_writes_private_envdir_files_and_restarts_service() {
    let tools = TempToolDir::new("runit-sync");
    tools.write_executable("sv", "#!/bin/sh\nexit 0\n");
    let service = tools.create_dir("services/unixnotis-daemon");
    let manager = ServiceManagerPaths {
        kind: ServiceManagerKind::Runit,
        artifact_root: service.parent().expect("service parent").to_path_buf(),
        live_root: None,
    };
    let _tools = use_fake_tool_bin(tools.path());

    sync_runit(&manager).expect("synchronize runit environment");

    let environment = service.join("env/WAYLAND_DISPLAY");
    assert!(environment.is_file());
    assert_eq!(
        fs::metadata(environment)
            .expect("environment metadata")
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
}
