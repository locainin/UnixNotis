use std::sync::Mutex;

use unixnotis_core::{ControlState, PopupGateState};

use super::super::state::{should_publish_cached, update_cached};

#[test]
fn cached_state_emits_first_value_then_suppresses_duplicates() {
    let cache = Mutex::new(None);
    let state = ControlState {
        dnd_enabled: false,
        dnd_expires_at: 0,
        history_count: 1,
        inhibited: false,
        inhibitor_count: 0,
    };

    // First value must be emitted because clients have no previous state
    assert!(should_publish_cached(&cache, &state));
    update_cached(&cache, state.clone());
    // Identical values should not wake D-Bus subscribers again
    assert!(!should_publish_cached(&cache, &state));
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

    assert!(should_publish_cached(&cache, &open));
    update_cached(&cache, open);
    // A changed popup gate affects visibility policy, so it must emit
    assert!(should_publish_cached(&cache, &dnd));
    update_cached(&cache, dnd.clone());
    assert!(!should_publish_cached(&cache, &dnd));
}

#[test]
fn cached_state_emits_after_counter_change() {
    let cache = Mutex::new(None);
    let first = ControlState {
        dnd_enabled: false,
        dnd_expires_at: 0,
        history_count: 0,
        inhibited: false,
        inhibitor_count: 0,
    };
    let changed = ControlState {
        history_count: 1,
        ..first
    };

    assert!(should_publish_cached(&cache, &first));
    update_cached(&cache, first);
    assert!(should_publish_cached(&cache, &changed));
    update_cached(&cache, changed.clone());
    assert!(!should_publish_cached(&cache, &changed));
}

#[test]
fn cached_state_emits_when_only_the_dnd_deadline_changes() {
    let cache = Mutex::new(None);
    let indefinite = ControlState {
        dnd_enabled: true,
        dnd_expires_at: 0,
        history_count: 0,
        inhibited: false,
        inhibitor_count: 0,
    };
    assert!(should_publish_cached(&cache, &indefinite));
    update_cached(&cache, indefinite.clone());

    let timed = ControlState {
        dnd_expires_at: 500,
        ..indefinite
    };

    assert!(should_publish_cached(&cache, &timed));
    update_cached(&cache, timed.clone());
    assert!(!should_publish_cached(&cache, &timed));
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

    assert!(should_publish_cached(&cache, &state));
    update_cached(&cache, state.clone());
    assert!(!should_publish_cached(&cache, &state));
}

#[test]
fn cached_state_is_not_advanced_until_success_is_recorded() {
    let cache = Mutex::new(None);
    let state = PopupGateState {
        dnd_enabled: true,
        inhibited: false,
    };

    assert!(should_publish_cached(&cache, &state));
    assert!(should_publish_cached(&cache, &state));

    update_cached(&cache, state.clone());
    assert!(!should_publish_cached(&cache, &state));
}
