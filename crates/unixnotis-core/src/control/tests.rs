use super::{
    popup_allowed_by_state, should_archive_closed_notification, CloseReason, ControlState,
};
use crate::Urgency;

#[test]
fn popup_gate_blocks_everything_when_inhibited() {
    let state = ControlState {
        inhibited: true,
        ..ControlState::default()
    };
    assert!(!popup_allowed_by_state(Urgency::Critical as u8, &state));
    assert!(!popup_allowed_by_state(Urgency::Normal as u8, &state));
}

#[test]
fn popup_gate_keeps_only_critical_during_dnd() {
    let state = ControlState {
        dnd_enabled: true,
        ..ControlState::default()
    };
    assert!(popup_allowed_by_state(Urgency::Critical as u8, &state));
    assert!(!popup_allowed_by_state(Urgency::Normal as u8, &state));
}

#[test]
fn user_dismiss_never_archives() {
    assert!(!should_archive_closed_notification(
        CloseReason::DismissedByUser,
        false,
        true
    ));
    assert!(!should_archive_closed_notification(
        CloseReason::DismissedByUser,
        true,
        true
    ));
}

#[test]
fn transient_archive_follows_config() {
    assert!(!should_archive_closed_notification(
        CloseReason::Expired,
        true,
        false
    ));
    assert!(should_archive_closed_notification(
        CloseReason::Expired,
        true,
        true
    ));
}

#[test]
fn non_transient_close_still_archives() {
    assert!(should_archive_closed_notification(
        CloseReason::Expired,
        false,
        false
    ));
    assert!(should_archive_closed_notification(
        CloseReason::ClosedByCall,
        false,
        true
    ));
}
