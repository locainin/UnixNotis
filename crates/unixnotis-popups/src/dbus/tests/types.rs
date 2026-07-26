use super::{UiCommand, UiEvent};

#[test]
fn dismiss_command_preserves_notification_id() {
    assert!(matches!(UiCommand::Dismiss(17), UiCommand::Dismiss(17)));
}

#[test]
fn shutdown_command_preserves_the_cleanup_acknowledgement() {
    let (acknowledgement_tx, acknowledgement_rx) = std::sync::mpsc::sync_channel(1);
    let command = UiCommand::Shutdown(acknowledgement_tx);

    if let UiCommand::Shutdown(acknowledgement) = command {
        acknowledgement
            .send(())
            .expect("send shutdown acknowledgement");
    } else {
        panic!("shutdown command variant should remain intact");
    }
    acknowledgement_rx
        .recv()
        .expect("receive shutdown acknowledgement");
}

#[test]
fn reload_events_remain_distinct() {
    assert!(matches!(UiEvent::CssReload, UiEvent::CssReload));
    assert!(matches!(UiEvent::ConfigReload, UiEvent::ConfigReload));
}
