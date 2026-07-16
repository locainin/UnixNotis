use unixnotis_core::{ControlState, PopupGateState};

use super::super::events::apply_popup_gate;

#[test]
fn popup_gate_update_changes_policy_without_replacing_runtime_counts() {
    let mut state = ControlState {
        dnd_enabled: false,
        inhibited: false,
        history_count: 42,
        inhibitor_count: 3,
    };

    apply_popup_gate(
        &mut state,
        PopupGateState {
            dnd_enabled: true,
            inhibited: true,
        },
    );

    assert!(state.dnd_enabled);
    assert!(state.inhibited);
    assert_eq!(state.history_count, 42);
    assert_eq!(state.inhibitor_count, 3);
}

#[test]
fn popup_gate_update_can_restore_normal_popup_policy() {
    let mut state = ControlState {
        dnd_enabled: true,
        inhibited: true,
        ..ControlState::default()
    };

    apply_popup_gate(
        &mut state,
        PopupGateState {
            dnd_enabled: false,
            inhibited: false,
        },
    );

    assert!(!state.dnd_enabled);
    assert!(!state.inhibited);
}
