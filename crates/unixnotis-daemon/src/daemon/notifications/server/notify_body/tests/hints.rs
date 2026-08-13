use std::collections::HashMap;

use zbus::zvariant::{OwnedValue, SerializeValue, Value};
use zbus::Message;

use super::super::{preflight_notify, PreflightError};
use super::support::notify_message;
use crate::daemon::notifications::server::notify_body::MAX_NOTIFY_WIRE_BODY_BYTES;

#[test]
fn hint_entry_flood_is_rejected_before_map_allocation() {
    let hints = (0..17)
        .map(|index| (format!("hint-{index}"), OwnedValue::from(index as u32)))
        .collect();
    let message = notify_message("app", "", "summary", "", Vec::new(), hints);

    assert_eq!(
        preflight_notify(&message),
        Err(PreflightError::LimitsExceeded(
            "Notify hint dictionary has too many entries"
        ))
    );
}

#[test]
fn contiguous_image_array_keeps_its_separate_large_allowance() {
    let message = notify_message_with_image(1024 * 1024);

    assert_eq!(preflight_notify(&message), Ok(()));
}

#[test]
fn image_array_above_native_allowance_is_rejected_below_the_wire_limit() {
    let message = notify_message_with_image(4 * 1024 * 1024 + 1);

    assert!(message.body().len() < MAX_NOTIFY_WIRE_BODY_BYTES);
    assert_eq!(
        preflight_notify(&message),
        Err(PreflightError::LimitsExceeded(
            "Notify byte array exceeds its allowance"
        ))
    );
}

fn notify_message_with_image(image_bytes: usize) -> Message {
    let image = (
        256_i32,
        256_i32,
        1024_i32,
        true,
        8_i32,
        4_i32,
        vec![0_u8; image_bytes],
    );
    let hints = HashMap::from([("image-data", SerializeValue(&image))]);

    Message::method("/org/freedesktop/Notifications", "Notify")
        .expect("method builder")
        .interface("org.freedesktop.Notifications")
        .expect("notification interface")
        .build(&(
            "app",
            0_u32,
            "",
            "summary",
            "",
            Vec::<String>::new(),
            hints,
            0_i32,
        ))
        .expect("Notify message")
}

#[test]
fn non_image_byte_array_does_not_inherit_the_image_allowance() {
    let mut hints = HashMap::new();
    hints.insert(
        "x-example-bytes".to_string(),
        OwnedValue::try_from(Value::from(vec![0_u8; 16 * 1024 + 1])).expect("owned byte hint"),
    );
    let message = notify_message("app", "", "summary", "", Vec::new(), hints);

    assert_eq!(
        preflight_notify(&message),
        Err(PreflightError::LimitsExceeded(
            "Notify byte array exceeds its allowance"
        ))
    );
}

#[test]
fn cumulative_nested_string_data_is_bounded() {
    let text = "h".repeat(crate::daemon::notifications::ingress::limits::MAX_HINT_STRING_BYTES);
    let hints = (0..16)
        .map(|index| {
            let values = Value::from(vec![text.as_str(); 4]);
            (
                format!("hint-{index}"),
                OwnedValue::try_from(values).expect("owned nested strings"),
            )
        })
        .collect();
    let message = notify_message("app", "", "summary", "", Vec::new(), hints);

    assert!(message.body().len() < MAX_NOTIFY_WIRE_BODY_BYTES);
    assert_eq!(
        preflight_notify(&message),
        Err(PreflightError::LimitsExceeded(
            "Notify contains too much non-image string data"
        ))
    );
}

#[test]
fn nested_non_image_array_fanout_is_bounded() {
    let nested = Value::from(vec!["x"; 65]);
    let mut hints = HashMap::new();
    hints.insert(
        "x-example-values".to_string(),
        OwnedValue::try_from(nested).expect("owned nested string array"),
    );
    let message = notify_message("app", "", "summary", "", Vec::new(), hints);

    assert_eq!(
        preflight_notify(&message),
        Err(PreflightError::LimitsExceeded(
            "Notify nested array has too many elements"
        ))
    );
}
