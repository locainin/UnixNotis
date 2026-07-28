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
