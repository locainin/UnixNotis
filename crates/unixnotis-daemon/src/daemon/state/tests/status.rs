use crate::test_support::daemon_state_for_test;
use std::sync::atomic::Ordering;

use super::super::DaemonState;

impl DaemonState {
    pub(crate) fn popups_running(&self) -> bool {
        // Test assertions observe the same sequentially consistent flag used by supervision
        self.popups_running.load(Ordering::SeqCst)
    }
}

#[tokio::test]
async fn daemon_state_boolean_flags_reflect_runtime_updates() {
    let state = daemon_state_for_test(true).await;

    assert!(state.trial_mode());
    assert!(!state.panel_ready());
    assert!(!state.popups_running());

    // These atomics gate user-visible command handling, so getters must reflect writes exactly
    state.set_panel_ready(true);
    state.set_popups_running(true);

    assert!(state.panel_ready());
    assert!(state.popups_running());
}

#[tokio::test]
async fn daemon_state_boolean_flags_can_return_to_false() {
    let state = daemon_state_for_test(true).await;

    state.set_panel_ready(true);
    state.set_popups_running(true);
    state.set_panel_ready(false);
    state.set_popups_running(false);

    assert!(!state.panel_ready());
    assert!(!state.popups_running());
}

#[tokio::test]
async fn daemon_state_trial_mode_can_be_disabled() {
    let state = daemon_state_for_test(false).await;

    // Trial mode changes control authorization, so false must stay observable
    assert!(!state.trial_mode());
}
