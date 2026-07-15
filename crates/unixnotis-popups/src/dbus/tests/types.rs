use super::{UiCommand, UiEvent};

#[test]
fn dismiss_command_preserves_notification_id() {
    assert!(matches!(UiCommand::Dismiss(17), UiCommand::Dismiss(17)));
}

#[test]
fn reload_events_remain_distinct() {
    assert!(matches!(UiEvent::CssReload, UiEvent::CssReload));
    assert!(matches!(UiEvent::ConfigReload, UiEvent::ConfigReload));
}
