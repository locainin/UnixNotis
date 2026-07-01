use tokio::sync::mpsc;

use super::drain_offline_commands;
use crate::dbus::UiCommand;

#[test]
fn drain_offline_commands_removes_all_queued_commands() {
    let (tx, mut rx) = mpsc::channel(4);
    tx.try_send(UiCommand::Dismiss(10))
        .expect("dismiss command should queue");
    tx.try_send(UiCommand::InvokeAction {
        id: 11,
        action_key: "default".to_string(),
    })
    .expect("action command should queue");

    drain_offline_commands(&mut rx);

    // Stale commands are intentionally discarded while popups are offline
    assert!(rx.try_recv().is_err());
}

#[test]
fn drain_offline_commands_accepts_empty_queue() {
    let (_tx, mut rx) = mpsc::channel(1);

    drain_offline_commands(&mut rx);

    assert!(rx.try_recv().is_err());
}
