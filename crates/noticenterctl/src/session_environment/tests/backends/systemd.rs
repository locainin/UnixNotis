use std::fs;

use super::super::super::backends::sync_systemd;
use super::super::support::TempToolDir;
use crate::system_tools::routing::use_fake_tool_bin;

#[test]
fn systemd_sync_runs_environment_import_and_restart_commands() {
    let tools = TempToolDir::new("systemd-sync");
    let log = tools.path().join("commands.log");
    for name in ["dbus-update-activation-environment", "systemctl"] {
        tools.write_executable(
            name,
            &format!(
                "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{}'\nexit 0\n",
                log.display()
            ),
        );
    }
    let _tools = use_fake_tool_bin(tools.path());

    sync_systemd().expect("synchronize systemd environment");

    let calls = fs::read_to_string(log).expect("read systemd command log");
    assert!(calls.contains("--user import-environment"));
    assert!(calls.contains("--user --no-block restart unixnotis-daemon.service"));
}
