use super::try_send_command;
use crate::dbus::UiCommand;
use unixnotis_core::NotificationKey;

#[test]
fn available_command_queue_receives_dismiss_without_delay() {
    let (tx, mut rx) = tokio::sync::mpsc::channel(1);

    let notification = NotificationKey {
        id: 42,
        generation: 5,
    };
    try_send_command(&tx, UiCommand::Dismiss(notification));

    assert!(matches!(
        rx.try_recv(),
        Ok(UiCommand::Dismiss(key)) if key == notification
    ));
}

#[test]
fn closed_command_queue_drops_action_without_panicking() {
    let (tx, rx) = tokio::sync::mpsc::channel(1);
    drop(rx);

    // A dead runtime must not crash the GTK click handler
    try_send_command(
        &tx,
        UiCommand::InvokeAction {
            notification: NotificationKey {
                id: 7,
                generation: 9,
            },
            action_key: "open".to_string(),
        },
    );
}
