use super::{popup_allowed_by_state, ControlState};
use crate::Urgency;

#[test]
fn popup_gate_blocks_everything_when_inhibited() {
    let state = ControlState {
        inhibited: true,
        ..ControlState::default()
    };

    // Inhibition is the strongest user intent, so even critical popups are hidden
    assert!(!popup_allowed_by_state(Urgency::Critical as u8, &state));
    assert!(!popup_allowed_by_state(Urgency::Normal as u8, &state));
}

#[test]
fn popup_gate_keeps_only_critical_during_dnd() {
    let state = ControlState {
        dnd_enabled: true,
        ..ControlState::default()
    };

    // DND keeps urgent notifications visible while suppressing normal chatter
    assert!(popup_allowed_by_state(Urgency::Critical as u8, &state));
    assert!(!popup_allowed_by_state(Urgency::Normal as u8, &state));
}
