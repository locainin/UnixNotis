//! Collapsed notification stack tests

use super::{layer_visibility, StackLayerVisibility};

#[test]
fn one_hidden_notification_uses_only_the_back_layer() {
    assert_eq!(
        layer_visibility(1),
        StackLayerVisibility {
            middle: false,
            back: true,
        }
    );
}

#[test]
fn two_or_more_hidden_notifications_use_both_rear_layers() {
    let expected = StackLayerVisibility {
        middle: true,
        back: true,
    };

    assert_eq!(layer_visibility(2), expected);
    assert_eq!(layer_visibility(u8::MAX), expected);
}

#[test]
fn expanded_or_single_notification_rows_hide_rear_layers() {
    assert_eq!(
        layer_visibility(0),
        StackLayerVisibility {
            middle: false,
            back: false,
        }
    );
}
