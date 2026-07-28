use std::process::{Command, Stdio};

use super::*;

#[test]
fn process_start_time_parser_handles_spaces_in_the_command_name() {
    let stat = "42 (daemon with spaces) S 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 987654 20";

    assert_eq!(parse_process_start_time(stat), Some(987_654));
}

#[test]
fn process_start_time_parser_rejects_missing_and_invalid_fields() {
    assert!(parse_process_start_time("42 missing-parenthesis").is_none());
    assert!(parse_process_start_time("42 (daemon) S 1 2 3").is_none());

    let invalid = "42 (daemon) S 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 invalid 20";
    assert!(parse_process_start_time(invalid).is_none());
}

#[test]
fn process_handle_rejects_a_mismatched_program_before_signaling() {
    let error = match ProcessHandle::open(std::process::id(), "not-the-test-process") {
        Ok(_) => panic!("mismatched program must fail closed"),
        Err(error) => error,
    };

    assert!(error
        .to_string()
        .contains("no longer matches expected daemon"));
}

#[test]
fn pidfd_signal_and_wait_stop_the_exact_child_process() {
    let mut child = Command::new("sleep")
        .arg("30")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn sleep child");
    let pid = child.id();

    let handle = match ProcessHandle::open(pid, "sleep").expect("open sleep process handle") {
        ProcessState::Running(handle) => handle,
        ProcessState::Gone => panic!("sleep child should still be running"),
    };
    handle.terminate().expect("terminate exact sleep child");
    handle.wait_for_exit().expect("wait for exact sleep child");

    let status = child.wait().expect("reap sleep child");
    assert!(!status.success());
}

#[test]
fn invalid_process_ids_are_treated_as_already_gone() {
    assert!(matches!(
        ProcessHandle::open(0, "daemon").expect("zero pid should be harmless"),
        ProcessState::Gone
    ));
    assert!(matches!(
        ProcessHandle::open(u32::MAX, "daemon").expect("oversized pid should be harmless"),
        ProcessState::Gone
    ));
}
