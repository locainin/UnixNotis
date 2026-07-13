//! Collapsed notification-stack composition tests

use super::build::{StackLayer, STACK_LAYER_ORDER};

#[test]
fn notification_stack_places_readable_card_above_rear_layers() {
    // The foreground must remain last because later GTK siblings paint on top
    assert_eq!(
        STACK_LAYER_ORDER,
        [StackLayer::Back, StackLayer::Middle, StackLayer::Foreground]
    );
}
