use crate::test_support::daemon_state_for_test;
use std::sync::atomic::Ordering;

use super::super::DaemonState;

impl DaemonState {
    pub(crate) fn popups_process_running(&self) -> bool {
        self.popups_process_running.load(Ordering::SeqCst)
    }
}

#[tokio::test]
async fn daemon_state_boolean_flags_reflect_runtime_updates() {
    let state = daemon_state_for_test(true).await;

    assert!(state.trial_mode());
    assert!(!state.panel_ready());
    assert!(!state.popups_process_running());

    // These atomics gate user-visible command handling, so getters must reflect writes exactly
    state.set_center_process_running(true);
    state.set_panel_ready(true);
    state.set_popups_process_running(true);

    assert!(state.panel_ready());
    assert!(state.popups_process_running());
    state.set_popups_ready(":1.10", true);

    let health = state.ui_health();
    assert!(health.center_process_running);
    assert!(health.center_ready);
    assert!(health.popups_process_running);
    assert!(health.popups_ready);
}

#[tokio::test]
async fn daemon_state_boolean_flags_can_return_to_false() {
    let state = daemon_state_for_test(true).await;

    state.set_panel_ready(true);
    state.set_center_process_running(true);
    state.set_popups_process_running(true);
    state.set_panel_ready(false);
    state.set_center_process_running(false);
    state.set_popups_process_running(false);

    assert!(!state.panel_ready());
    assert!(!state.popups_process_running());
}

#[tokio::test]
async fn popup_readiness_can_only_be_cleared_by_its_owner_generation() {
    let state = daemon_state_for_test(true).await;
    state.set_popups_process_running(true);
    state.set_popups_ready(":1.10", true);

    state.set_popups_ready(":1.11", false);
    assert!(state.popups_ready());

    state.set_popups_ready(":1.10", false);
    assert!(!state.popups_ready());
}

#[tokio::test]
async fn stopped_popup_process_clears_composite_readiness() {
    let state = daemon_state_for_test(true).await;
    state.set_popups_process_running(true);
    state.set_popups_ready(":1.10", true);

    state.set_popups_process_running(false);

    let health = state.ui_health();
    assert!(!health.popups_process_running);
    assert!(!health.popups_ready);
}

#[tokio::test]
async fn daemon_state_trial_mode_can_be_disabled() {
    let state = daemon_state_for_test(false).await;

    // Trial mode changes control authorization, so false must stay observable
    assert!(!state.trial_mode());
}
