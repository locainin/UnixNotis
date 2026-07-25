use std::collections::HashMap;

use zbus::zvariant::{OwnedValue, Value};
use zbus::Message;

use super::super::{preflight_notify, PreflightError};
use super::notify_message;

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
fn notify_method_with_the_wrong_body_signature_is_rejected() {
    let message = Message::method("/org/freedesktop/Notifications", "Notify")
        .expect("method builder")
        .interface("org.freedesktop.Notifications")
        .expect("notification interface")
        .build(&("only-one-field",))
        .expect("wrong-signature message");

    assert_eq!(
        preflight_notify(&message),
        Err(PreflightError::Malformed("Notify has an invalid signature"))
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
