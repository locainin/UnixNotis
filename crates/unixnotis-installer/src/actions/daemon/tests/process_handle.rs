use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use super::*;

const CHILD_EXEC_TIMEOUT: Duration = Duration::from_secs(2);
const CHILD_EXEC_POLL_INTERVAL: Duration = Duration::from_millis(1);

fn spawn_ready_sleep_child() -> Child {
    let sleep = unixnotis_core::util::trusted_system_program_path("sleep")
        .expect("find sleep in a trusted system directory");
    let mut child = Command::new(sleep)
        .arg("30")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn sleep child");
    let deadline = Instant::now() + CHILD_EXEC_TIMEOUT;

    // Command::spawn can return before the child replaces the test executable
    while !process_matches_program(child.id(), "sleep") {
        match child.try_wait() {
            Ok(Some(status)) => panic!("sleep child exited before exec completed: {status}"),
            Ok(None) => {}
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                panic!("inspect sleep child before exec completed: {error}");
            }
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("sleep child did not complete exec within {CHILD_EXEC_TIMEOUT:?}");
        }
        std::thread::sleep(CHILD_EXEC_POLL_INTERVAL);
    }

    child
}

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
    let Err(error) = ProcessHandle::open(std::process::id(), "not-the-test-process") else {
        panic!("mismatched program must fail closed");
    };

    assert!(error
        .to_string()
        .contains("no longer matches expected daemon"));
}

#[test]
fn fallback_poll_budget_rounds_up_and_keeps_zero_immediate() {
    assert_eq!(fallback_poll_count(Duration::ZERO), 0);
    assert_eq!(fallback_poll_count(Duration::from_nanos(1)), 1);
    assert_eq!(fallback_poll_count(FALLBACK_POLL_INTERVAL), 1);
    assert_eq!(
        fallback_poll_count(FALLBACK_POLL_INTERVAL + Duration::from_nanos(1)),
        2
    );
}

#[test]
fn proc_comm_reader_reports_the_live_name_and_rejects_missing_processes() {
    let expected = std::fs::read_to_string("/proc/self/comm")
        .expect("read current process comm")
        .trim()
        .to_string();

    assert_eq!(
        read_proc_comm(std::process::id()).as_deref(),
        Some(expected.as_str())
    );
    assert_eq!(read_proc_comm(i32::MAX as u32), None);
}

#[test]
fn proc_comm_parser_rejects_blank_names_and_trims_kernel_newlines() {
    assert_eq!(
        parse_proc_comm("unixnotis-daemon\n").as_deref(),
        Some("unixnotis-daemon")
    );
    assert_eq!(parse_proc_comm(" \n\t"), None);
}

#[test]
fn pidfd_signal_and_wait_stop_the_exact_child_process() {
    let mut child = spawn_ready_sleep_child();
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

#[test]
fn current_process_start_time_is_read_from_proc() {
    let start_time = read_process_start_time(std::process::id())
        .expect("read current process state")
        .expect("current process should exist");

    assert!(start_time > 1);
}

#[test]
fn missing_process_start_time_returns_none() {
    assert_eq!(
        read_process_start_time(i32::MAX as u32).expect("missing process is not an I/O failure"),
        None
    );
}

#[test]
fn only_not_found_process_state_errors_mean_the_process_exited() {
    assert!(process_state_is_missing(&std::io::Error::from(
        std::io::ErrorKind::NotFound
    )));
    assert!(!process_state_is_missing(&std::io::Error::from(
        std::io::ErrorKind::PermissionDenied
    )));
}

#[test]
fn fallback_lifetime_check_accepts_current_and_rejects_stale_start_times() {
    let raw_pid = std::process::id();
    let pid = process_id(raw_pid).expect("current process id");
    let start_time = read_process_start_time(raw_pid)
        .expect("read current process state")
        .expect("current process should exist");
    let current = ProcessHandle {
        pid,
        start_time,
        pidfd: None,
        exit_timeout: Duration::from_millis(10),
    };
    let stale = ProcessHandle {
        pid,
        start_time: start_time.saturating_add(1),
        pidfd: None,
        exit_timeout: Duration::from_millis(10),
    };

    current
        .require_current_lifetime()
        .expect("matching fallback lifetime");
    assert!(current.wait_for_exit().is_err());
    assert!(stale.require_current_lifetime().is_err());
    stale
        .wait_for_exit()
        .expect("a different lifetime means the original process exited");
}

#[test]
fn pidfd_wait_times_out_while_the_exact_process_is_still_running() {
    let mut child = spawn_ready_sleep_child();
    let mut handle = match ProcessHandle::open(child.id(), "sleep").expect("open sleep handle") {
        ProcessState::Running(handle) => handle,
        ProcessState::Gone => panic!("sleep child should still be running"),
    };
    handle.exit_timeout = Duration::from_millis(10);

    assert!(handle.wait_for_exit().is_err());

    child.kill().expect("stop sleep child");
    child.wait().expect("reap sleep child");
}

#[test]
fn non_process_io_errors_are_not_collapsed_into_a_missing_process() {
    let root = std::env::temp_dir().join(format!(
        "unixnotis-process-state-directory-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("create process state directory");

    let result = read_process_start_time_from_path(&root);

    let _ = std::fs::remove_dir(&root);
    assert!(result.is_err());
}
