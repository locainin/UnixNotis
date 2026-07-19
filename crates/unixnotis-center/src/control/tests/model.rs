use super::{UiCommand, UiEvent};

#[test]
fn dismiss_command_preserves_notification_id() {
    assert!(matches!(UiCommand::Dismiss(29), UiCommand::Dismiss(29)));
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
        text: "private reply text".to_string(),
        outcome,
    };

    let rendered = format!("{command:?}");

    assert!(rendered.contains("Reply"));
    assert!(rendered.contains('9'));
    assert!(rendered.contains("[redacted]"));
    assert!(!rendered.contains("private reply text"));
}
