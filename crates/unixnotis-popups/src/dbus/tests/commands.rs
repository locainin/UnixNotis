use tokio::sync::mpsc;
use unixnotis_core::NotificationKey;

use super::drain_offline_commands;
use crate::dbus::UiCommand;

#[test]
fn drain_offline_commands_removes_all_queued_commands() {
    let (tx, mut rx) = mpsc::channel(4);
    tx.try_send(UiCommand::Dismiss(NotificationKey {
        id: 10,
        generation: 12,
    }))
    .expect("dismiss command should queue");
    tx.try_send(UiCommand::InvokeAction {
        notification: NotificationKey {
            id: 11,
            generation: 13,
        },
        action_key: "default".to_string(),
        confirmed: false,
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

#[test]
fn drain_offline_commands_reports_reply_delivery_failure() {
    let (tx, mut rx) = mpsc::channel(1);
    let (outcome, mut result) = tokio::sync::oneshot::channel();
    tx.try_send(UiCommand::Reply {
        id: 10,
        generation: 12,
        text: "Keep this private".to_string(),
        outcome,
    })
    .expect("reply command should queue");

    assert!(drain_offline_commands(&mut rx).is_none());
    assert_eq!(
        result.try_recv().expect("reply result should be ready"),
        Err("notification service is unavailable".to_string())
    );
}
