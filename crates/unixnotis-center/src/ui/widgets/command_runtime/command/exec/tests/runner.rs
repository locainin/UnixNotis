use std::io;
use std::time::Duration;

use unixnotis_core::CommandSpec;

use super::{build_command_runtime, run_command_with_timeout};

#[test]
fn blocking_runner_preserves_literal_direct_arguments() {
    let command = CommandSpec::direct("printf", ["battery|charging"]);

    let output = run_command_with_timeout(&command, Duration::ZERO, None)
        .expect("run direct command without a deadline");

    assert!(output.status.success());
    assert_eq!(output.stdout, b"battery|charging");
}

#[test]
fn asynchronous_runner_preserves_stdout_and_stderr() {
    let runtime = build_command_runtime().expect("build command runtime");
    let command = CommandSpec::shell("printf output; printf error >&2");

    let output = run_command_with_timeout(&command, Duration::from_secs(1), Some(&runtime))
        .expect("run command with Tokio pipe draining");

    assert!(output.status.success());
    assert_eq!(output.stdout, b"output");
    assert_eq!(output.stderr, b"error");
}

#[test]
fn blocking_runner_terminates_a_command_after_its_deadline() {
    let command = CommandSpec::shell("sleep 2");

    let error = run_command_with_timeout(&command, Duration::from_millis(20), None)
        .expect_err("blocking command should time out");

    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
}

#[test]
fn asynchronous_runner_terminates_a_command_after_its_deadline() {
    let runtime = build_command_runtime().expect("build command runtime");
    let command = CommandSpec::shell("sleep 2");

    let error = run_command_with_timeout(&command, Duration::from_millis(20), Some(&runtime))
        .expect_err("asynchronous command should time out");

    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
}
