use std::sync::Mutex;

use unixnotis_core::{ControlState, PopupGateState};

use super::super::signals::cached_state_would_emit;

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
    assert!(cached_state_would_emit(&cache, &state));
    // Identical values should not wake D-Bus subscribers again
    assert!(!cached_state_would_emit(&cache, &state));
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

    assert!(cached_state_would_emit(&cache, &open));
    // A changed popup gate affects visibility policy, so it must emit
    assert!(cached_state_would_emit(&cache, &dnd));
    assert!(!cached_state_would_emit(&cache, &dnd));
}

#[test]
fn cached_state_emits_after_counter_change() {
    let cache = Mutex::new(None);
    let first = ControlState {
        dnd_enabled: false,
        history_count: 0,
        inhibited: false,
        inhibitor_count: 0,
    };
    let changed = ControlState {
        history_count: 1,
        ..first
    };

    assert!(cached_state_would_emit(&cache, &first));
    assert!(cached_state_would_emit(&cache, &changed));
    assert!(!cached_state_would_emit(&cache, &changed));
}

#[test]
fn cached_state_recovers_from_poisoned_mutex() {
    let cache = Mutex::new(None);
    let _ = std::panic::catch_unwind(|| {
        let _guard = cache.lock().expect("lock before poison");
        panic!("poison cache");
    });

    let state = PopupGateState {
        dnd_enabled: false,
        inhibited: true,
    };

    assert!(cached_state_would_emit(&cache, &state));
    assert!(!cached_state_would_emit(&cache, &state));
}
