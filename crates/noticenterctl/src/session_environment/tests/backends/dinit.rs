use std::fs;

use super::super::super::backends::sync_dinit;
use super::super::support::TempToolDir;
use crate::system_tools::routing::use_fake_tool_bin;

#[test]
fn dinit_sync_tolerates_an_unstarted_restart_before_starting_service() {
    let tools = TempToolDir::new("dinit-sync");
    let log = tools.path().join("commands.log");
    tools.write_executable(
        "dinitctl",
        &format!(
            "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{}'\nif [ \"$2\" = \"restart\" ]; then exit 1; fi\nexit 0\n",
            log.display()
        ),
    );
    let _tools = use_fake_tool_bin(tools.path());

    sync_dinit().expect("synchronize dinit environment");

    let calls = fs::read_to_string(log).expect("read dinit command log");
    assert!(calls.contains("--user setenv"));
    assert!(calls.contains("--user restart --ignore-unstarted unixnotis-daemon"));
    assert!(calls.contains("--user start unixnotis-daemon"));
}
