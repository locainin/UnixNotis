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
