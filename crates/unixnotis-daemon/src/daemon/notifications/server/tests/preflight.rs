use std::collections::HashMap;

use zbus::zvariant::{OwnedValue, Structure, Value};
use zbus::Message;

use super::{preflight_notify, PreflightError};
use crate::daemon::notifications::server::ingress::MAX_NOTIFY_WIRE_BODY_BYTES;

fn notify_message(
    app_name: &str,
    app_icon: &str,
    summary: &str,
    body: &str,
    actions: Vec<String>,
    hints: HashMap<String, OwnedValue>,
) -> Message {
    Message::method("/org/freedesktop/Notifications", "Notify")
        .expect("method builder")
        .interface("org.freedesktop.Notifications")
        .expect("notification interface")
        .build(&(
            app_name, 0_u32, app_icon, summary, body, actions, hints, 0_i32,
        ))
        .expect("Notify message")
}

#[test]
fn ordinary_notify_body_passes_structural_preflight() {
    let message = notify_message(
        "Example",
        "example",
        "Summary",
        "Body",
        vec!["default".to_string(), "Open".to_string()],
        HashMap::new(),
    );

    assert_eq!(preflight_notify(&message), Ok(()));
}

#[test]
fn under_wire_limit_tiny_action_flood_is_rejected() {
    let actions = (0..20_000).map(|_| "a".to_string()).collect();
    let message = notify_message("app", "", "summary", "", actions, HashMap::new());

    assert!(message.body().len() < MAX_NOTIFY_WIRE_BODY_BYTES);
    assert_eq!(
        preflight_notify(&message),
        Err(PreflightError::LimitsExceeded(
            "Notify action array has too many elements"
        ))
    );
}

#[test]
fn action_array_accepts_eight_pairs_and_rejects_the_next_element() {
    let exact = vec!["a".to_string(); 16];
    let exact_message = notify_message("app", "", "", "", exact, HashMap::new());
    assert_eq!(preflight_notify(&exact_message), Ok(()));

    let over = vec!["a".to_string(); 17];
    let over_message = notify_message("app", "", "", "", over, HashMap::new());
    assert_eq!(
        preflight_notify(&over_message),
        Err(PreflightError::LimitsExceeded(
            "Notify action array has too many elements"
        ))
    );
}

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
fn field_string_limit_is_enforced_before_owned_string_creation() {
    let summary = "s".repeat(crate::daemon::notifications::limits::MAX_SUMMARY_BYTES + 1);
    let message = notify_message("app", "", &summary, "", Vec::new(), HashMap::new());

    assert_eq!(
        preflight_notify(&message),
        Err(PreflightError::LimitsExceeded(
            "Notify string exceeds its field limit"
        ))
    );
}

#[test]
fn contiguous_image_array_keeps_its_separate_large_allowance() {
    let image = Structure::from((
        256_i32,
        256_i32,
        1024_i32,
        true,
        8_i32,
        4_i32,
        vec![0_u8; 256 * 1024],
    ));
    let mut hints = HashMap::new();
    hints.insert(
        "image-data".to_string(),
        OwnedValue::try_from(Value::from(image)).expect("owned image hint"),
    );
    let message = notify_message("app", "", "summary", "", Vec::new(), hints);

    assert_eq!(preflight_notify(&message), Ok(()));
}

#[test]
fn image_array_above_its_allowance_is_rejected_below_the_wire_limit() {
    let image = Structure::from((
        256_i32,
        256_i32,
        1024_i32,
        true,
        8_i32,
        4_i32,
        vec![0_u8; 256 * 1024 + 1],
    ));
    let mut hints = HashMap::new();
    hints.insert(
        "image-data".to_string(),
        OwnedValue::try_from(Value::from(image)).expect("owned image hint"),
    );
    let message = notify_message("app", "", "summary", "", Vec::new(), hints);

    assert!(message.body().len() < MAX_NOTIFY_WIRE_BODY_BYTES);
    assert_eq!(
        preflight_notify(&message),
        Err(PreflightError::LimitsExceeded(
            "Notify byte array exceeds its allowance"
        ))
    );
}

#[test]
fn cumulative_nested_string_data_is_bounded() {
    let text = "h".repeat(crate::daemon::notifications::limits::MAX_HINT_STRING_BYTES);
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
fn cumulative_string_budget_accepts_its_exact_limit() {
    let hints = (0..16)
        .map(|index| {
            // Hint keys consume 38 bytes, so one shorter value keeps the total at 64 KiB
            let first_length = if index == 0 { 2_010 } else { 2_048 };
            let values = Value::from(vec!["h".repeat(first_length), "h".repeat(2_048)]);
            (
                format!("h{index}"),
                OwnedValue::try_from(values).expect("owned exact-budget strings"),
            )
        })
        .collect();
    let message = notify_message("", "", "", "", Vec::new(), hints);

    assert_eq!(preflight_notify(&message), Ok(()));
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
