use super::try_send_command;
use crate::dbus::UiCommand;

#[test]
fn available_command_queue_receives_dismiss_without_delay() {
    let (tx, mut rx) = tokio::sync::mpsc::channel(1);

    try_send_command(&tx, UiCommand::Dismiss(42));

    assert!(matches!(rx.try_recv(), Ok(UiCommand::Dismiss(42))));
}

#[test]
fn closed_command_queue_drops_action_without_panicking() {
    let (tx, rx) = tokio::sync::mpsc::channel(1);
    drop(rx);

    // A dead runtime must not crash the GTK click handler
    try_send_command(
        &tx,
        UiCommand::InvokeAction {
            id: 7,
            action_key: "open".to_string(),
        },
    );
}
