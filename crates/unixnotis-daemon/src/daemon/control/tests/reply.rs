use std::collections::HashMap;
use std::time::Duration;

use chrono::Utc;
use futures_util::TryStreamExt;
use unixnotis_core::{InlineReply, Notification, NotificationImage, Urgency};
use zbus::message::Type;
use zbus::zvariant::OwnedValue;
use zbus::{Connection, MatchRule, MessageStream};

use super::super::reply::{sanitize_reply_text, MAX_REPLY_TEXT_BYTES};
use super::super::ControlServer;
use crate::daemon::NOTIFICATIONS_OBJECT_PATH;
use crate::test_support::daemon_state_for_test;

#[test]
fn sanitize_reply_text_keeps_normal_text_and_trims_outer_spacing() {
    assert_eq!(
        sanitize_reply_text("  See you soon  ").expect("valid reply"),
        "See you soon"
    );
}

#[test]
fn sanitize_reply_text_rejects_empty_control_only_and_oversized_values() {
    assert!(sanitize_reply_text(" \n\t ").is_err());
    assert!(sanitize_reply_text("\u{202e}").is_err());
    assert!(sanitize_reply_text(&"x".repeat(MAX_REPLY_TEXT_BYTES + 1)).is_err());
}

#[tokio::test]
async fn submit_inline_reply_emits_text_and_removes_nonresident_notification() {
    let state = daemon_state_for_test(false).await;
    let mut stream = reply_signal_stream(&state).await;
    let id = {
        let mut store = state.store.lock().await;
        store.insert(reply_notification(false), 0).notification.id
    };

    ControlServer::new(state.clone())
        .submit_inline_reply(id, "  On my way  ")
        .await
        .expect("submit live inline reply");

    let (signal_id, text) = next_reply_signal(&mut stream).await;
    assert_eq!(signal_id, id);
    assert_eq!(text, "On my way");
    assert!(state.store.lock().await.list_active().is_empty());
    assert!(state.store.lock().await.list_history().is_empty());
}

#[tokio::test]
async fn submit_inline_reply_keeps_resident_notification_live() {
    let state = daemon_state_for_test(false).await;
    let mut stream = reply_signal_stream(&state).await;
    let id = {
        let mut store = state.store.lock().await;
        store.insert(reply_notification(true), 0).notification.id
    };

    ControlServer::new(state.clone())
        .submit_inline_reply(id, "Another update")
        .await
        .expect("submit resident inline reply");

    let (signal_id, text) = next_reply_signal(&mut stream).await;
    assert_eq!(signal_id, id);
    assert_eq!(text, "Another update");
    assert_eq!(state.store.lock().await.list_active().len(), 1);
}

fn reply_notification(is_resident: bool) -> Notification {
    Notification {
        id: 0,
        app_name: "Messages".to_string(),
        app_icon: String::new(),
        summary: "New message".to_string(),
        body: "Are you coming?".to_string(),
        actions: vec![unixnotis_core::Action {
            key: "inline-reply".to_string(),
            label: "Reply".to_string(),
        }],
        inline_reply: InlineReply {
            available: true,
            label: "Reply".to_string(),
            ..InlineReply::default()
        },
        hints: HashMap::<String, OwnedValue>::new(),
        urgency: Urgency::Normal,
        category: None,
        is_transient: false,
        is_resident,
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

async fn reply_signal_stream(state: &crate::daemon::DaemonState) -> MessageStream {
    let receiver = Connection::session().await.expect("receiver session bus");
    let sender = state
        .connection()
        .unique_name()
        .expect("daemon connection has unique name")
        .to_string();
    let rule = MatchRule::builder()
        .msg_type(Type::Signal)
        .sender(sender.as_str())
        .expect("signal sender")
        .path(NOTIFICATIONS_OBJECT_PATH)
        .expect("notification object path")
        .interface("org.freedesktop.Notifications")
        .expect("notification interface")
        .member("NotificationReplied")
        .expect("reply member")
        .build();
    MessageStream::for_match_rule(rule, &receiver, Some(4))
        .await
        .expect("reply signal stream")
}

async fn next_reply_signal(stream: &mut MessageStream) -> (u32, String) {
    let signal = tokio::time::timeout(Duration::from_millis(500), stream.try_next())
        .await
        .expect("reply signal should arrive before timeout")
        .expect("reply signal stream should stay open")
        .expect("reply signal");
    signal
        .body()
        .deserialize::<(u32, String)>()
        .expect("reply signal body")
}
