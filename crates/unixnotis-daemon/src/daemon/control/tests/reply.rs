use std::collections::HashMap;
use std::time::Duration;

use chrono::Utc;
use futures_util::TryStreamExt;
use unixnotis_core::{InlineReply, Notification, NotificationImage, Urgency};
use zbus::fdo::DBusProxy;
use zbus::message::Type;
use zbus::zvariant::OwnedValue;
use zbus::{Connection, MatchRule, MessageStream};

use super::super::ControlServer;
use super::{validate_reply_text, MAX_REPLY_TEXT_BYTES};
use crate::daemon::NOTIFICATIONS_OBJECT_PATH;
use crate::test_support::daemon_state_for_test;

#[test]
fn validate_reply_text_keeps_message_content_and_trims_outer_spacing() {
    assert_eq!(
        validate_reply_text("  See you soon  ").expect("valid reply"),
        "See you soon"
    );
}

#[test]
fn validate_reply_text_preserves_unicode_and_bidirectional_content_exactly() {
    let messages = [
        "مرحبًا، سأصل قريبًا",
        "שלום, אגיע בקרוב",
        "Reply 👩🏽‍💻 cafe\u{301}",
        "English \u{2067}مرحبا שלום\u{2069} English",
    ];

    for message in messages {
        assert_eq!(
            validate_reply_text(message).expect("valid Unicode"),
            message
        );
    }
}

#[test]
fn validate_reply_text_accepts_exact_byte_limit() {
    let reply = "🙂".repeat(MAX_REPLY_TEXT_BYTES / "🙂".len());

    assert_eq!(reply.len(), MAX_REPLY_TEXT_BYTES);
    assert_eq!(validate_reply_text(&reply).expect("exact limit"), reply);
}

#[test]
fn validate_reply_text_rejects_empty_oversized_nul_and_multiline_values() {
    assert!(validate_reply_text(" \n\t ").is_err());
    assert!(validate_reply_text(&"x".repeat(MAX_REPLY_TEXT_BYTES + 1)).is_err());
    assert!(validate_reply_text("before\0after").is_err());
    assert!(validate_reply_text("line one\nline two").is_err());
}

#[tokio::test]
async fn submit_inline_reply_emits_text_and_removes_nonresident_notification() {
    let state = daemon_state_for_test(false).await;
    let sender = Connection::session().await.expect("sender session bus");
    let mut stream = reply_signal_stream(&state, &sender).await;
    let id = {
        let mut store = state.store.lock().await;
        store
            .insert(reply_notification(false, &sender), 0)
            .notification
            .id
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
    let sender = Connection::session().await.expect("sender session bus");
    let mut stream = reply_signal_stream(&state, &sender).await;
    let id = {
        let mut store = state.store.lock().await;
        store
            .insert(reply_notification(true, &sender), 0)
            .notification
            .id
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

#[tokio::test]
async fn submit_inline_reply_round_trips_unicode_and_exact_byte_limit() {
    let state = daemon_state_for_test(false).await;
    let sender = Connection::session().await.expect("sender session bus");
    let mut stream = reply_signal_stream(&state, &sender).await;
    let messages = [
        "مرحبًا، سأصل قريبًا".to_string(),
        "שלום, אגיע בקרוב".to_string(),
        "Reply 👩🏽‍💻 cafe\u{301}".to_string(),
        "English \u{2067}مرحبا שלום\u{2069} English".to_string(),
        "🙂".repeat(MAX_REPLY_TEXT_BYTES / "🙂".len()),
    ];

    for message in messages {
        let id = {
            let mut store = state.store.lock().await;
            store
                .insert(reply_notification(true, &sender), 0)
                .notification
                .id
        };

        ControlServer::new(state.clone())
            .submit_inline_reply(id, &message)
            .await
            .expect("submit exact reply text");

        let (signal_id, signal_text) = next_reply_signal(&mut stream).await;
        assert_eq!(signal_id, id);
        assert_eq!(signal_text, message);
    }
}

#[tokio::test]
async fn reply_listener_replacement_survives_generation_safe_dismissal() {
    let state = daemon_state_for_test(false).await;
    let sender = Connection::session().await.expect("sender session bus");
    let mut stream = reply_signal_stream(&state, &sender).await;
    let id = {
        let mut store = state.store.lock().await;
        store
            .insert(reply_notification(false, &sender), 0)
            .notification
            .id
    };
    let replacement_state = state.clone();
    let replacement_sender = sender.clone();

    ControlServer::new(state.clone())
        .submit_inline_reply_with_post_emit(id, "yes", move || async move {
            // This models the sender updating the same row while handling the reply signal
            let (signal_id, text) = next_reply_signal(&mut stream).await;
            assert_eq!((signal_id, text.as_str()), (id, "yes"));
            let mut replacement = reply_notification(false, &replacement_sender);
            replacement.summary = "Reply received".to_string();
            let outcome = replacement_state.store.lock().await.insert(replacement, id);
            assert!(outcome.replaced);
        })
        .await
        .expect("reply with replacement");

    let active = state
        .store
        .lock()
        .await
        .active_notification_view(id)
        .expect("same-ID replacement should remain active");
    assert_eq!(active.summary, "Reply received");
}

#[tokio::test]
async fn reply_listener_close_removes_replied_notification_without_history() {
    let state = daemon_state_for_test(false).await;
    let sender = Connection::session().await.expect("sender session bus");
    let mut stream = reply_signal_stream(&state, &sender).await;
    let id = {
        let mut store = state.store.lock().await;
        store
            .insert(reply_notification(false, &sender), 0)
            .notification
            .id
    };
    let closing_state = state.clone();

    ControlServer::new(state.clone())
        .submit_inline_reply_with_post_emit(id, "yes", move || async move {
            let (signal_id, text) = next_reply_signal(&mut stream).await;
            assert_eq!((signal_id, text.as_str()), (id, "yes"));
            closing_state
                .close_notification(id, unixnotis_core::CloseReason::ClosedByCall)
                .await
                .expect("sender close should succeed");
        })
        .await
        .expect("reply with sender close");

    let store = state.store.lock().await;
    assert!(store.list_active().is_empty());
    assert!(store.list_history().is_empty());
}

#[tokio::test]
async fn submit_inline_reply_rejects_sender_that_no_longer_owns_bus_name() {
    let state = daemon_state_for_test(false).await;
    let sender = Connection::session().await.expect("sender session bus");
    let id = {
        let mut store = state.store.lock().await;
        store
            .insert(reply_notification(false, &sender), 0)
            .notification
            .id
    };
    let sender_name = sender.unique_name().expect("sender unique name").clone();
    sender.close().await.expect("close sender connection");
    let proxy = DBusProxy::new(state.connection())
        .await
        .expect("create bus proxy");
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            let has_owner = proxy
                .name_has_owner(sender_name.clone().into())
                .await
                .expect("query sender ownership");
            if !has_owner {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("bus should release the closed sender name");

    let error = ControlServer::new(state.clone())
        .submit_inline_reply(id, "Anyone there?")
        .await
        .expect_err("closed sender must reject replies");

    assert!(error
        .to_string()
        .contains("The application is no longer available"));
    assert!(state
        .store
        .lock()
        .await
        .active_notification_view(id)
        .is_some());
}

#[tokio::test]
async fn inline_reply_signal_reaches_owner_but_not_unrelated_observer() {
    let state = daemon_state_for_test(false).await;
    let owner = Connection::session().await.expect("owner session bus");
    let observer = Connection::session().await.expect("observer session bus");
    let mut owner_stream = reply_signal_stream(&state, &owner).await;
    let mut observer_stream = reply_signal_stream(&state, &observer).await;
    let id = {
        let mut store = state.store.lock().await;
        store
            .insert(reply_notification(true, &owner), 0)
            .notification
            .id
    };

    ControlServer::new(state)
        .submit_inline_reply(id, "private reply")
        .await
        .expect("submit owner reply");

    assert_eq!(
        next_reply_signal(&mut owner_stream).await,
        (id, "private reply".to_string())
    );
    assert_no_reply_signal(&mut observer_stream).await;
}

fn reply_notification(is_resident: bool, sender: &Connection) -> Notification {
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
        sender_name: Some(
            sender
                .unique_name()
                .expect("sender connection unique name")
                .to_string(),
        ),
        sender_pid: Some(1234),
        sender_start_time: Some(555),
        sender_executable: Some("/usr/bin/test-app".to_string()),
    }
}

async fn reply_signal_stream(
    state: &crate::daemon::DaemonState,
    receiver: &Connection,
) -> MessageStream {
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
    MessageStream::for_match_rule(rule, receiver, Some(4))
        .await
        .expect("reply signal stream")
}

async fn assert_no_reply_signal(stream: &mut MessageStream) {
    assert!(
        tokio::time::timeout(Duration::from_millis(100), stream.try_next())
            .await
            .is_err(),
        "unrelated observer must not receive reply text"
    );
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
