use super::try_send_command;
use crate::control::UiCommand;

#[test]
fn available_command_queue_receives_the_original_command() {
    let (command_tx, mut command_rx) = tokio::sync::mpsc::channel(1);

    try_send_command(&command_tx, UiCommand::SetDnd(true));

    assert_eq!(command_rx.try_recv(), Ok(UiCommand::SetDnd(true)));
}

#[test]
fn closed_command_queue_drops_the_command_without_panicking() {
    let (command_tx, command_rx) = tokio::sync::mpsc::channel(1);
    drop(command_rx);

    try_send_command(&command_tx, UiCommand::ClearAll);
}
