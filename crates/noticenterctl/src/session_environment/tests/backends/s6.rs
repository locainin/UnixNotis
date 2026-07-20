use unixnotis_core::service_manager::{ServiceManagerKind, ServiceManagerPaths};

use super::super::super::backends::sync_s6;
use super::super::support::TempToolDir;
use crate::system_tools::routing::use_fake_tool_bin;

#[test]
fn s6_sync_writes_envdir_files_and_addresses_the_resolved_live_tree() {
    let tools = TempToolDir::new("s6-sync");
    for name in ["s6-rc", "s6-svc"] {
        tools.write_executable(name, "#!/bin/sh\nexit 0\n");
    }
    let service = tools.create_dir("s6/sv/unixnotis-daemon");
    let live = tools.create_dir("live");
    let manager = ServiceManagerPaths {
        kind: ServiceManagerKind::S6,
        artifact_root: tools.path().join("s6"),
        live_root: Some(live),
    };
    let _tools = use_fake_tool_bin(tools.path());

    sync_s6(&manager).expect("synchronize s6 environment");

    assert!(service.join("env/XDG_RUNTIME_DIR").is_file());
}
