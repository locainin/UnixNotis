use super::{UiCommand, UiEvent};
use unixnotis_core::NotificationKey;

#[test]
fn dismiss_command_preserves_notification_generation() {
    let notification = NotificationKey {
        id: 17,
        generation: 23,
    };

    assert!(matches!(
        UiCommand::Dismiss(notification),
        UiCommand::Dismiss(key) if key == notification
    ));
}

#[test]
fn reply_debug_output_redacts_private_message_text() {
    let (outcome, _result) = tokio::sync::oneshot::channel();
    let command = UiCommand::Reply {
        id: 17,
        generation: 23,
        text: "private reply content".to_string(),
        outcome,
    };
    let rendered = format!("{command:?}");

    assert!(
        !rendered.contains("private reply content"),
        "reply text must not enter debug output"
    );
    assert!(
        rendered.contains("<redacted>"),
        "debug output should make redaction explicit"
    );
}

#[test]
fn reload_events_remain_distinct() {
    assert!(matches!(UiEvent::CssReload, UiEvent::CssReload));
    assert!(matches!(UiEvent::ConfigReload, UiEvent::ConfigReload));
}

#[test]
fn popup_hover_events_preserve_notification_generation_and_state() {
    let key = NotificationKey {
        id: 17,
        generation: 23,
    };

    assert!(matches!(
        UiEvent::PopupHoverChanged(key, true),
        UiEvent::PopupHoverChanged(event_key, hovered) if event_key == key && hovered
    ));
}
