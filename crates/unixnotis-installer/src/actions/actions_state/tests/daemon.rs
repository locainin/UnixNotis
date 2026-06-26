use super::systemd_stop_error_is_satisfied_by_state;

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
