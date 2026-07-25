use std::collections::HashMap;
use std::time::Duration;

use chrono::Utc;
use unixnotis_core::{CloseReason, Notification, NotificationImage, Urgency};
use zbus::zvariant::OwnedValue;
use zbus::Message;

use super::super::ControlServer;
use crate::expire::{ExpirationCommand, ExpirationScheduler};
use crate::test_support::daemon_state_for_test;

fn notification(summary: &str) -> Notification {
    Notification {
        id: 0,
        app_name: "TestApp".to_string(),
        app_icon: String::new(),
        attribution: unixnotis_core::NotificationAttribution::default(),
        summary: summary.to_string(),
        body: String::new(),
        actions: Vec::new(),
        inline_reply: unixnotis_core::InlineReply::default(),
        inline_reply_policy: unixnotis_core::InlineReplyPolicy::Allow,
        hints: HashMap::<String, OwnedValue>::new(),
        urgency: Urgency::Normal,
        category: None,
        is_transient: false,
        is_resident: false,
        suppress_popup: false,
        suppress_sound: false,
        image: NotificationImage::default(),
        expire_timeout: 0,
        received_at: Utc::now(),
        sender_name: Some(":1.test".to_string()),
        sender_pid: Some(1234),
        sender_start_time: Some(555),
        sender_executable: Some("/usr/bin/test-app".to_string()),
    }
}

fn control_header_message(method: &str) -> Message {
    Message::method("/com/unixnotis/Control", method)
        .expect("method builder")
        .interface("com.unixnotis.Control")
        .expect("interface")
        .sender(":1.4242")
        .expect("sender")
        .build(&())
        .expect("message")
}

async fn next_cancel_id(
    receiver: &mut tokio::sync::mpsc::UnboundedReceiver<ExpirationCommand>,
) -> u32 {
    let command = tokio::time::timeout(Duration::from_millis(100), receiver.recv())
        .await
        .expect("cancel command should arrive")
        .expect("scheduler channel should stay open");
    match command {
        ExpirationCommand::Cancel { id } => id,
        ExpirationCommand::Schedule { .. } => panic!("clear should cancel expiration"),
    }
}

#[tokio::test]
async fn drain_active_notifications_returns_ids_and_cancels_expirations() {
    let state = daemon_state_for_test(false).await;
    let (scheduler, mut receiver) = ExpirationScheduler::channel_for_test();
    state.set_scheduler(scheduler);
    let server = ControlServer::new(state.clone());
    let ids = {
        let mut store = state.store.lock().await;
        let first = store.insert(notification("first"), 0).notification.id;
        let second = store.insert(notification("second"), 0).notification.id;
        vec![second, first]
    };

    let drained = server.drain_active_notifications().await;

    assert_eq!(drained, ids);
    assert_eq!(next_cancel_id(&mut receiver).await, ids[0]);
    assert_eq!(next_cancel_id(&mut receiver).await, ids[1]);
    assert!(state.store.lock().await.list_active().is_empty());
}

#[tokio::test]
async fn clear_saved_history_removes_archived_notifications() {
    let state = daemon_state_for_test(false).await;
    let server = ControlServer::new(state.clone());
    let id = {
        let mut store = state.store.lock().await;
        let id = store.insert(notification("history"), 0).notification.id;
        store.close(id, CloseReason::Undefined);
        id
    };
    assert!(state
        .store
        .lock()
        .await
        .list_history()
        .into_iter()
        .any(|view| view.id == id));

    server.clear_saved_history().await;

    assert!(state
        .store
        .lock()
        .await
        .list_history()
        .into_iter()
        .all(|view| view.id != id));
}

#[tokio::test]
async fn clear_all_rejects_unauthorized_sender_before_mutating_state() {
    let state = daemon_state_for_test(false).await;
    let server = ControlServer::new(state.clone());
    {
        let mut store = state.store.lock().await;
        store.insert(notification("active"), 0);
    }
    let message = control_header_message("ClearAll");

    server
        .clear_all(message.header())
        .await
        .expect_err("unauthorized clear all should fail");

    assert_eq!(state.store.lock().await.list_active().len(), 1);
}

#[tokio::test]
async fn clear_active_rejects_unauthorized_sender_before_mutating_state() {
    let state = daemon_state_for_test(false).await;
    let server = ControlServer::new(state.clone());
    {
        let mut store = state.store.lock().await;
        store.insert(notification("active"), 0);
    }
    let message = control_header_message("ClearActive");

    server
        .clear_active(message.header())
        .await
        .expect_err("unauthorized clear active should fail");

    assert_eq!(state.store.lock().await.list_active().len(), 1);
}

#[tokio::test]
async fn clear_history_rejects_unauthorized_sender_before_mutating_state() {
    let state = daemon_state_for_test(false).await;
    let server = ControlServer::new(state.clone());
    let id = {
        let mut store = state.store.lock().await;
        let id = store.insert(notification("history"), 0).notification.id;
        store.close(id, CloseReason::Undefined);
        id
    };
    let message = control_header_message("ClearHistory");

    server
        .clear_history(message.header())
        .await
        .expect_err("unauthorized clear history should fail");

    assert!(state
        .store
        .lock()
        .await
        .list_history()
        .into_iter()
        .any(|view| view.id == id));
}

#[tokio::test]
async fn invoke_action_rejects_unauthorized_sender_before_signal_emit() {
    let state = daemon_state_for_test(false).await;
    let server = ControlServer::new(state);
    let message = control_header_message("InvokeAction");

    server
        .invoke_action(7, "default", message.header())
        .await
        .expect_err("unauthorized action should fail");
}

#[tokio::test]
async fn timed_dnd_rejects_unauthorized_sender_before_mutating_state() {
    let state = daemon_state_for_test(false).await;
    let server = ControlServer::new(state.clone());
    let message = control_header_message("SetDndUntil");

    server
        .set_dnd_until(Utc::now().timestamp() + 600, message.header())
        .await
        .expect_err("unauthorized timed DND should fail");

    assert!(!state.store.lock().await.dnd_enabled());
}

#[tokio::test]
async fn inline_reply_rejects_unauthorized_sender_before_live_state_lookup() {
    let state = daemon_state_for_test(false).await;
    let server = ControlServer::new(state);
    let message = control_header_message("ReplyNotification");

    server
        .reply_notification(7, "private text", message.header())
        .await
        .expect_err("unauthorized inline reply should fail");
}
