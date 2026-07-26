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

    assert!(drain_offline_commands(&mut rx).is_none());

    // Stale commands are intentionally discarded while popups are offline
    assert!(rx.try_recv().is_err());
}

#[test]
fn drain_offline_commands_accepts_empty_queue() {
    let (_tx, mut rx) = mpsc::channel(1);

    assert!(drain_offline_commands(&mut rx).is_none());

    assert!(rx.try_recv().is_err());
}

#[test]
fn drain_offline_commands_returns_shutdown_acknowledgement() {
    let (tx, mut rx) = mpsc::channel(1);
    let (acknowledgement_tx, acknowledgement_rx) = std::sync::mpsc::sync_channel(1);
    tx.try_send(UiCommand::Shutdown(acknowledgement_tx))
        .expect("shutdown command should queue");

    let acknowledgement =
        drain_offline_commands(&mut rx).expect("shutdown acknowledgement should be preserved");
    acknowledgement.send(()).expect("acknowledge shutdown");

    acknowledgement_rx
        .recv()
        .expect("receive shutdown acknowledgement");
}
