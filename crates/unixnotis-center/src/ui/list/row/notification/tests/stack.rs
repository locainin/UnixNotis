//! Collapsed notification-stack composition tests

use super::build::{StackLayer, STACK_LAYER_ORDER};
use super::update::{stack_ghost_visibility, StackGhostVisibility};

#[test]
fn notification_stack_places_readable_card_above_rear_layers() {
    // The foreground must remain last because later GTK siblings paint on top
    assert_eq!(
        STACK_LAYER_ORDER,
        [StackLayer::Back, StackLayer::Middle, StackLayer::Foreground]
    );
}

#[test]
fn two_notification_stack_uses_non_overlapping_back_slot() {
    assert_eq!(
        stack_ghost_visibility(1),
        StackGhostVisibility {
            middle: false,
            back: true,
        }
    );
}

#[test]
fn three_notification_stack_uses_both_rear_slots() {
    assert_eq!(
        stack_ghost_visibility(2),
        StackGhostVisibility {
            middle: true,
            back: true,
        }
    );

    // Larger groups remain capped to the same two visual depth layers
    assert_eq!(stack_ghost_visibility(u8::MAX), stack_ghost_visibility(2));
}

#[test]
fn single_notification_stack_hides_both_rear_slots() {
    assert_eq!(
        stack_ghost_visibility(0),
        StackGhostVisibility {
            middle: false,
            back: false,
        }
    );
}
