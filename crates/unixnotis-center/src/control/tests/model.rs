use super::{UiCommand, UiEvent};
use unixnotis_core::NotificationKey;

#[test]
fn dismiss_command_preserves_notification_generation() {
    let notification = NotificationKey {
        id: 29,
        generation: 31,
    };

    assert!(matches!(
        UiCommand::Dismiss(notification),
        UiCommand::Dismiss(key) if key == notification
    ));
}

#[test]
fn reload_events_remain_distinct() {
    assert!(matches!(UiEvent::CssReload, UiEvent::CssReload));
    assert!(matches!(UiEvent::ConfigReload, UiEvent::ConfigReload));
}

#[test]
fn reply_command_debug_output_redacts_the_typed_message() {
    let (outcome, _result) = tokio::sync::oneshot::channel();
    let command = UiCommand::Reply {
        id: 9,
        generation: 12,
        text: "private reply text".to_string(),
        outcome,
    };

    let rendered = format!("{command:?}");

    assert!(rendered.contains("Reply"));
    assert!(rendered.contains('9'));
    assert!(rendered.contains("[redacted]"));
    assert!(!rendered.contains("private reply text"));
}
