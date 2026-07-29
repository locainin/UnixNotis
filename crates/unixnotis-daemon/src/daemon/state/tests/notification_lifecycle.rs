use std::collections::HashMap;
use std::time::Duration;

use chrono::Utc;
use unixnotis_core::{CloseReason, Notification, NotificationImage, Urgency};
use zbus::zvariant::OwnedValue;

use crate::expire::{ExpirationCommand, ExpirationScheduler};
use crate::test_support::daemon_state_for_test;

fn notification(summary: &str) -> Notification {
    Notification {
        id: 0,
        generation: 0,
        app_name: "TestApp".to_string(),
        app_icon: String::new(),
        attribution: unixnotis_core::NotificationAttribution::default(),
        attribution_diagnostics: unixnotis_core::AttributionDiagnostics::default(),
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

async fn next_cancel_id(
    receiver: &mut tokio::sync::mpsc::UnboundedReceiver<ExpirationCommand>,
) -> u32 {
    let command = tokio::time::timeout(Duration::from_millis(100), receiver.recv())
        .await
        .expect("cancel command should arrive")
        .expect("scheduler channel should stay open");
    match command {
        ExpirationCommand::Cancel { id, .. } => id,
        ExpirationCommand::Schedule { .. } => panic!("dismiss should cancel expiration"),
    }
}

#[tokio::test]
async fn generation_dismiss_removes_matching_history_without_canceling_timer() {
    let state = daemon_state_for_test(false).await;
    let (scheduler, mut receiver) = ExpirationScheduler::channel_for_test();
    state.set_scheduler(scheduler);
    let key = {
        let mut store = state.store.lock().await;
        let inserted = store.insert(notification("history"), 0);
        let key = inserted.notification.key();
        store.close(key.id, CloseReason::Expired);
        key
    };

    state
        .dismiss_generation(key)
        .await
        .expect("matching history generation dismiss should succeed");

    assert!(receiver.try_recv().is_err());
    assert!(state
        .store
        .lock()
        .await
        .list_history()
        .into_iter()
        .all(|view| view.key() != key));
}

#[tokio::test]
async fn generation_dismiss_rejects_a_missing_notification() {
    let state = daemon_state_for_test(false).await;
    let (scheduler, mut receiver) = ExpirationScheduler::channel_for_test();
    state.set_scheduler(scheduler);

    state
        .dismiss_generation(unixnotis_core::NotificationKey {
            id: 999,
            generation: 1,
        })
        .await
        .expect_err("missing generation dismiss should fail");

    assert!(receiver.try_recv().is_err());
}

#[tokio::test]
async fn generation_safe_dismiss_keeps_replacement_and_its_timer() {
    let state = daemon_state_for_test(false).await;
    let (scheduler, mut receiver) = ExpirationScheduler::channel_for_test();
    state.set_scheduler(scheduler);
    let (id, original) = {
        let mut store = state.store.lock().await;
        let original = store.insert(notification("original"), 0).notification;
        let id = original.id;
        let replacement = store.insert(notification("replacement"), id);
        assert!(replacement.replaced);
        (id, original)
    };

    let removed = state
        .dismiss_replied_if_current(id, &original)
        .await
        .expect("stale generation dismiss should remain a no-op");

    assert!(!removed);
    assert!(receiver.try_recv().is_err());
    let active = state
        .store
        .lock()
        .await
        .active_notification_view(id)
        .expect("replacement should remain active");
    assert_eq!(active.summary, "replacement");
}

#[tokio::test]
async fn generation_safe_panel_dismiss_rejects_a_stale_same_id_generation() {
    let state = daemon_state_for_test(false).await;
    let (scheduler, mut receiver) = ExpirationScheduler::channel_for_test();
    state.set_scheduler(scheduler);
    let (stale_key, replacement_key) = {
        let mut store = state.store.lock().await;
        let original = store.insert(notification("original"), 0).notification;
        let replacement = store
            .insert(notification("replacement"), original.id)
            .notification;
        (original.key(), replacement.key())
    };

    state
        .dismiss_generation(stale_key)
        .await
        .expect_err("stale generation dismiss should fail");

    assert!(receiver.try_recv().is_err());
    assert_eq!(
        state
            .store
            .lock()
            .await
            .active_notification_view(replacement_key.id)
            .expect("replacement should remain active")
            .key(),
        replacement_key
    );
}

#[tokio::test]
async fn generation_safe_panel_dismiss_removes_and_cancels_the_current_generation() {
    let state = daemon_state_for_test(false).await;
    let (scheduler, mut receiver) = ExpirationScheduler::channel_for_test();
    state.set_scheduler(scheduler);
    let key = state
        .store
        .lock()
        .await
        .insert(notification("current"), 0)
        .notification
        .key();

    state
        .dismiss_generation(key)
        .await
        .expect("current generation dismiss should succeed");

    assert_eq!(next_cancel_id(&mut receiver).await, key.id);
    assert!(state
        .store
        .lock()
        .await
        .active_notification_view(key.id)
        .is_none());
}

#[tokio::test]
async fn close_notification_removes_active_notification_and_cancels_timer() {
    let state = daemon_state_for_test(false).await;
    let (scheduler, mut receiver) = ExpirationScheduler::channel_for_test();
    state.set_scheduler(scheduler);
    let id = {
        let mut store = state.store.lock().await;
        store.insert(notification("close"), 0).notification.id
    };

    state
        .close_notification(id, CloseReason::ClosedByCall)
        .await
        .expect("close should succeed");

    assert_eq!(next_cancel_id(&mut receiver).await, id);
    assert!(state
        .store
        .lock()
        .await
        .active_notification_view(id)
        .is_none());
}

#[tokio::test]
async fn close_notification_missing_id_is_noop() {
    let state = daemon_state_for_test(false).await;
    let (scheduler, mut receiver) = ExpirationScheduler::channel_for_test();
    state.set_scheduler(scheduler);

    state
        .close_notification(777, CloseReason::ClosedByCall)
        .await
        .expect("missing close should succeed");

    assert!(receiver.try_recv().is_err());
}
