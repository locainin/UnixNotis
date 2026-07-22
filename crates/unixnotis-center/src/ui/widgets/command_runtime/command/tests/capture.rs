use std::io::ErrorKind;
use std::time::Duration;

use super::super::test_support::configure_command_test_root;
use super::{run_command_capture_async, run_command_capture_with_timeout_async};
use unixnotis_core::CommandSpec;

#[test]
fn empty_capture_command_returns_invalid_input_without_enqueueing() {
    let response = run_command_capture_async(&CommandSpec::direct("", [] as [&str; 0]))
        .recv_blocking()
        .expect("capture response should remain available")
        .expect_err("empty command should fail");

    assert_eq!(response.kind(), ErrorKind::InvalidInput);
}

#[test]
fn custom_capture_timeout_terminates_long_running_command() {
    configure_command_test_root();
    let response = run_command_capture_with_timeout_async(
        &CommandSpec::direct("sleep", ["1"]),
        Duration::from_millis(40),
    )
    .recv_blocking()
    .expect("capture response should remain available")
    .expect_err("sleep should exceed the custom timeout");

    assert_eq!(response.kind(), ErrorKind::TimedOut);
}
