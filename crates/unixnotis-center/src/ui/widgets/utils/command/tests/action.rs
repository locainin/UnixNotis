use std::io::ErrorKind;

use super::run_command_capture_action_async;

#[test]
fn empty_action_command_returns_invalid_input_without_enqueueing() {
    let response = run_command_capture_action_async("  ")
        .recv_blocking()
        .expect("action response should remain available")
        .expect_err("empty command should fail");

    assert_eq!(response.kind(), ErrorKind::InvalidInput);
}

#[test]
fn action_command_runs_in_the_action_lane_and_reports_output() {
    let output = run_command_capture_action_async("true")
        .recv_blocking()
        .expect("action response should remain available")
        .expect("true should execute");

    assert!(output.status.success());
}
