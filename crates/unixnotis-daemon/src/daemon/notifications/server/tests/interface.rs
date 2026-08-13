use std::collections::HashMap;

use zbus::Message;

use super::NotificationServer;
use crate::expire::ExpirationScheduler;
use crate::test_support::daemon_state_for_test;

fn notify_header_message() -> Message {
    Message::method("/org/freedesktop/Notifications", "Notify")
        .expect("method builder")
        .interface("org.freedesktop.Notifications")
        .expect("interface")
        .sender(":1.42")
        .expect("sender")
        .build(&())
        .expect("message")
}

#[tokio::test]
async fn get_capabilities_returns_freedesktop_capability_contract() {
    let state = daemon_state_for_test(false).await;
    let scheduler = ExpirationScheduler::start(state.clone());
    let server = NotificationServer::new(state, scheduler);

    let capabilities = server.get_capabilities().await;

    assert!(capabilities.contains(&"actions".to_string()));
    assert!(capabilities.contains(&"body".to_string()));
    assert!(!capabilities.contains(&"body-markup".to_string()));
    assert!(capabilities.contains(&"icon-static".to_string()));
    assert!(!capabilities.contains(&"xyzzy".to_string()));
}

#[tokio::test]
async fn get_server_information_returns_stable_identity_and_spec_version() {
    let state = daemon_state_for_test(false).await;
    let scheduler = ExpirationScheduler::start(state.clone());
    let server = NotificationServer::new(state, scheduler);

    let info = server.get_server_information().await;

    assert_eq!(
        info,
        (
            "UnixNotis".to_string(),
            "UnixNotis".to_string(),
            env!("CARGO_PKG_VERSION").to_string(),
            "1.2".to_string()
        )
    );
}

#[tokio::test]
async fn notify_wrapper_stores_notification_and_returns_assigned_id() {
    let state = daemon_state_for_test(false).await;
    let scheduler = ExpirationScheduler::start(state.clone());
    let server = NotificationServer::new(state.clone(), scheduler);
    let message = notify_header_message();
    let header = message.header();

    let id = server
        .notify(
            "app".to_string(),
            0,
            String::new(),
            "summary".to_string(),
            "body".to_string(),
            Vec::new(),
            HashMap::new().into(),
            header.clone(),
            0,
        )
        .await
        .expect("notify should store");

    assert_eq!(id, 1);
    let active = state
        .store
        .lock()
        .await
        .active_notification_view(id)
        .expect("notification should be active");
    assert_eq!(active.summary, "summary");
}

#[tokio::test]
async fn close_notification_wrapper_removes_owned_active_notification() {
    let state = daemon_state_for_test(false).await;
    let scheduler = ExpirationScheduler::start(state.clone());
    let server = NotificationServer::new(state.clone(), scheduler);
    let message = notify_header_message();
    let header = message.header();
    let id = server
        .notify(
            "app".to_string(),
            0,
            String::new(),
            "summary".to_string(),
            "body".to_string(),
            Vec::new(),
            HashMap::new().into(),
            header.clone(),
            0,
        )
        .await
        .expect("notify should store");

    server
        .close_notification(id, header)
        .await
        .expect("owned close should succeed");

    assert!(state
        .store
        .lock()
        .await
        .active_notification_view(id)
        .is_none());
}
