use std::io::ErrorKind;

use super::super::test_support::configure_command_test_root;
use super::run_command_capture_action_async;
use unixnotis_core::CommandSpec;

#[test]
fn empty_action_command_returns_invalid_input_without_enqueueing() {
    let response = run_command_capture_action_async(&CommandSpec::direct("", [] as [&str; 0]))
        .recv_blocking()
        .expect("action response should remain available")
        .expect_err("empty command should fail");

    assert_eq!(response.kind(), ErrorKind::InvalidInput);
}

#[test]
fn action_command_runs_in_the_action_lane_and_reports_output() {
    configure_command_test_root();
    let output = run_command_capture_action_async(&CommandSpec::direct("true", [] as [&str; 0]))
        .recv_blocking()
        .expect("action response should remain available")
        .expect("true should execute");

    assert!(output.status.success());
}
