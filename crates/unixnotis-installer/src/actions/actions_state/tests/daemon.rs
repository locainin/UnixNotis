use super::{pid_alive, pid_matches_comm, systemd_stop_error_is_satisfied_by_state};

#[test]
fn systemd_stop_error_can_continue_when_unit_is_inactive() {
    // A failed stop is acceptable only when systemd reports a non-running state
    assert!(systemd_stop_error_is_satisfied_by_state("inactive"));
}

#[test]
fn systemd_stop_error_can_continue_when_unit_is_failed() {
    // Failed units no longer own the notification bus, so reinstall may continue
    assert!(systemd_stop_error_is_satisfied_by_state("failed"));
}

#[test]
fn systemd_stop_error_still_fails_when_unit_stays_active() {
    assert!(!systemd_stop_error_is_satisfied_by_state("active"));
}

#[test]
fn systemd_stop_error_still_fails_when_unit_is_transitioning() {
    assert!(!systemd_stop_error_is_satisfied_by_state("deactivating"));
}

#[test]
fn systemd_stop_error_still_fails_when_state_is_empty() {
    // Empty output means the manager did not provide enough proof that stopping succeeded
    assert!(!systemd_stop_error_is_satisfied_by_state(""));
}

#[test]
fn systemd_stop_error_trims_state_output_before_matching() {
    // systemctl prints a trailing newline in normal output
    assert!(systemd_stop_error_is_satisfied_by_state(" inactive\n"));
    assert!(systemd_stop_error_is_satisfied_by_state("\tunknown "));
}

#[test]
fn systemd_stop_error_rejects_unrecognized_non_running_words() {
    // Only explicit systemd states should satisfy a failed stop
    assert!(!systemd_stop_error_is_satisfied_by_state("dead"));
    assert!(!systemd_stop_error_is_satisfied_by_state("stopped"));
}

#[test]
fn pid_alive_reports_current_process_as_alive() {
    let pid = std::process::id();

    // The current test process should always satisfy a kill -0 probe
    assert!(pid_alive(pid).expect("current pid probe"));
}

#[test]
fn pid_alive_reports_impossible_pid_as_not_alive() {
    let alive = pid_alive(u32::MAX).expect("invalid pid probe should still run");

    // A non-existent PID must not be treated as safe to signal
    assert!(!alive);
}

#[test]
fn pid_matches_comm_rejects_wrong_process_name() {
    let pid = std::process::id();

    let matches = pid_matches_comm(pid, "definitely-not-unixnotis").expect("comm probe");

    // PID reuse protection depends on rejecting mismatched command names
    assert!(!matches);
}
