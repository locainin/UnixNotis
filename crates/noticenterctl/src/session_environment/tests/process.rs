use unixnotis_core::CommandSpec;

use super::super::process::{require_success, run};
use super::support::TempToolDir;
use crate::system_tools::routing::use_fake_tool_bin;

#[test]
fn run_executes_a_resolved_direct_service_command() {
    let tools = TempToolDir::new("process-success");
    tools.write_executable("service-tool", "#!/bin/sh\nexit 0\n");
    let _tools = use_fake_tool_bin(tools.path());

    let status = run(&CommandSpec::direct("service-tool", ["literal|argument"]))
        .expect("run direct service command");

    assert!(status.success());
}

#[test]
fn require_success_reports_a_failed_service_command() {
    let tools = TempToolDir::new("process-failure");
    tools.write_executable("service-tool", "#!/bin/sh\nexit 23\n");
    let _tools = use_fake_tool_bin(tools.path());

    let error = require_success(&CommandSpec::direct("service-tool", [] as [&str; 0]))
        .expect_err("failed service command must be rejected");

    assert!(error.to_string().contains("status"));
}

#[test]
fn run_rejects_shell_service_commands() {
    let error = run(&CommandSpec::shell("service-tool | parser"))
        .expect_err("service commands must remain direct");

    assert!(error.to_string().contains("resolve trusted"));
}
