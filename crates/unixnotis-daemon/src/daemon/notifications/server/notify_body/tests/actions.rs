use std::collections::HashMap;

use super::super::{preflight_notify, PreflightError};
use super::support::notify_message;
use crate::daemon::notifications::server::ingress::MAX_NOTIFY_WIRE_BODY_BYTES;

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
fn action_key_and_label_keep_independent_field_limits() {
    let oversized_key =
        "k".repeat(crate::daemon::notifications::ingress::limits::MAX_ACTION_KEY_BYTES + 1);
    let key_message = notify_message(
        "app",
        "",
        "",
        "",
        vec![oversized_key, "label".to_string()],
        HashMap::new(),
    );
    assert_eq!(
        preflight_notify(&key_message),
        Err(PreflightError::LimitsExceeded(
            "Notify string exceeds its field limit"
        ))
    );

    let oversized_label =
        "l".repeat(crate::daemon::notifications::ingress::limits::MAX_ACTION_LABEL_BYTES + 1);
    let label_message = notify_message(
        "app",
        "",
        "",
        "",
        vec!["key".to_string(), oversized_label],
        HashMap::new(),
    );
    assert_eq!(
        preflight_notify(&label_message),
        Err(PreflightError::LimitsExceeded(
            "Notify string exceeds its field limit"
        ))
    );
}
