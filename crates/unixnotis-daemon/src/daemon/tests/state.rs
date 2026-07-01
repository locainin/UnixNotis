use std::sync::Mutex;

use unixnotis_core::{ControlState, PopupGateState};

use crate::test_support::daemon_state_for_test;

use super::should_emit_cached;

#[test]
fn cached_state_emits_first_value_then_suppresses_duplicates() {
    let cache = Mutex::new(None);
    let state = ControlState {
        dnd_enabled: false,
        history_count: 1,
        inhibited: false,
        inhibitor_count: 0,
    };

    // First value must be emitted because clients have no previous state
    assert!(should_emit_cached(&cache, &state));
    // Identical values should not wake D-Bus subscribers again
    assert!(!should_emit_cached(&cache, &state));
}

#[test]
fn cached_state_emits_when_any_gate_field_changes() {
    let cache = Mutex::new(None);
    let open = PopupGateState {
        dnd_enabled: false,
        inhibited: false,
    };
    let dnd = PopupGateState {
        dnd_enabled: true,
        inhibited: false,
    };

    assert!(should_emit_cached(&cache, &open));
    // A changed popup gate affects visibility policy, so it must emit
    assert!(should_emit_cached(&cache, &dnd));
    assert!(!should_emit_cached(&cache, &dnd));
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
async fn daemon_state_trial_mode_can_be_disabled() {
    let state = daemon_state_for_test(false).await;

    // Trial mode changes control authorization, so false must stay observable
    assert!(!state.trial_mode());
}
