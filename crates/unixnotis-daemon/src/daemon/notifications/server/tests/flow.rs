//! Notification server flow tests

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use futures_util::TryStreamExt;
use tracing::Level;
use tracing_subscriber::filter::LevelFilter;
use unixnotis_core::{
    CloseReason, Config, Notification, NotificationImage, Urgency, CONTROL_OBJECT_PATH,
};
use zbus::message::Type;
use zbus::{Connection, MatchRule, Message, MessageStream};

use crate::daemon::{DaemonState, NotificationServer};
use crate::expire::ExpirationScheduler;
use crate::sound::SoundSettings;
use crate::store::{InsertOutcome, NotificationStore};
use crate::test_support::daemon_state_for_test;

fn notification_with_id(id: u32) -> Arc<Notification> {
    Arc::new(Notification {
        id,
        app_name: "app".to_string(),
        app_icon: String::new(),
        attribution: unixnotis_core::NotificationAttribution::default(),
        summary: "summary".to_string(),
        body: String::new(),
        actions: Vec::new(),
        inline_reply: unixnotis_core::InlineReply::default(),
        inline_reply_policy: unixnotis_core::InlineReplyPolicy::Allow,
        hints: HashMap::new(),
        urgency: Urgency::Normal,
        category: None,
        is_transient: false,
        is_resident: false,
        suppress_popup: false,
        suppress_sound: false,
        image: NotificationImage::default(),
        expire_timeout: -1,
        received_at: Utc::now(),
        sender_name: Some(":1.test".to_string()),
        sender_pid: Some(42),
        sender_start_time: Some(77),
        sender_executable: Some("/usr/bin/test-app".to_string()),
    })
}

fn insert_outcome(id: u32, dropped: bool) -> InsertOutcome {
    InsertOutcome {
        notification: notification_with_id(id),
        replaced: false,
        show_popup: !dropped,
        allow_sound: !dropped,
        evicted: Vec::new(),
        dropped,
    }
}

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

async fn daemon_state_with_config(config: Config) -> Arc<DaemonState> {
    let connection = Connection::session().await.expect("session bus");
    let sound = SoundSettings::from_config(&config, None);
    let store = NotificationStore::new_with_state_store(config, None);
    DaemonState::new_with_store(connection, store, sound, false)
}

async fn control_signal_stream(state: &DaemonState, member: &str) -> MessageStream {
    let receiver = Connection::session().await.expect("receiver session bus");
    let sender = state
        .connection()
        .unique_name()
        .expect("daemon connection has unique name")
        .to_string();
    let rule = MatchRule::builder()
        .msg_type(Type::Signal)
        .sender(sender.as_str())
        .expect("sender")
        .path(CONTROL_OBJECT_PATH)
        .expect("path")
        .interface("com.unixnotis.Control")
        .expect("interface")
        .member(member)
        .expect("member")
        .build();
    MessageStream::for_match_rule(rule, &receiver, Some(8))
        .await
        .expect("signal stream")
}

async fn next_signal(stream: &mut MessageStream) -> Message {
    tokio::time::timeout(Duration::from_millis(500), stream.try_next())
        .await
        .expect("signal should arrive before timeout")
        .expect("signal stream should stay open")
        .expect("signal message")
}

#[test]
fn handle_dropped_notification_returns_id_for_dropped_payload() {
    let outcome = insert_outcome(9, true);

    let id = NotificationServer::handle_dropped_notification(&outcome);

    assert_eq!(id, Some(9));
}

#[test]
fn handle_dropped_notification_returns_none_for_stored_payload() {
    let outcome = insert_outcome(9, false);

    let id = NotificationServer::handle_dropped_notification(&outcome);

    assert_eq!(id, None);
}

#[test]
fn log_received_notification_reports_false_when_debug_is_disabled() {
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(LevelFilter::INFO)
        .finish();

    let logged = tracing::subscriber::with_default(subscriber, || {
        NotificationServer::log_received_notification("app", "summary", "body", 0, 100)
    });

    assert!(!logged);
}

#[test]
fn log_received_notification_reports_true_when_debug_is_enabled() {
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(Level::DEBUG)
        .finish();

    let logged = tracing::subscriber::with_default(subscriber, || {
        NotificationServer::log_received_notification("app", "summary", "body", 0, 100)
    });

    assert!(logged);
}

#[tokio::test]
async fn ingest_notify_stores_notifications_and_returns_assigned_ids() {
    let state = daemon_state_for_test(false).await;
    let scheduler = ExpirationScheduler::start(state.clone());
    let server = NotificationServer::new(state.clone(), scheduler);
    let message = notify_header_message();
    let header = message.header();

    let id = server
        .ingest_notify(
            "app".to_string(),
            0,
            String::new(),
            "summary".to_string(),
            "body".to_string(),
            Vec::new(),
            HashMap::new(),
            &header,
            0,
        )
        .await
        .expect("notify should store");
    let second_id = server
        .ingest_notify(
            "app".to_string(),
            0,
            String::new(),
            "next".to_string(),
            "body".to_string(),
            Vec::new(),
            HashMap::new(),
            &header,
            0,
        )
        .await
        .expect("second notify should store");

    let store = state.store.lock().await;
    let active = store.active_notification_view(id).expect("active view");
    assert_eq!(id, 1);
    assert_eq!(second_id, 2);
    assert_eq!(active.id, id);
    assert_eq!(active.summary, "summary");
}

#[tokio::test]
async fn ingest_notify_schedules_expiration_for_positive_timeout() {
    let state = daemon_state_for_test(false).await;
    let scheduler = ExpirationScheduler::start(state.clone());
    let server = NotificationServer::new(state.clone(), scheduler);
    let message = notify_header_message();
    let header = message.header();

    let id = server
        .ingest_notify(
            "app".to_string(),
            0,
            String::new(),
            "expires".to_string(),
            "body".to_string(),
            Vec::new(),
            HashMap::new(),
            &header,
            25,
        )
        .await
        .expect("notify should store");

    for _ in 0..30 {
        if state
            .store
            .lock()
            .await
            .active_notification_view(id)
            .is_none()
        {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    panic!("notification should expire after scheduled timeout");
}

#[tokio::test]
async fn ingest_notify_emits_notification_added_signal() {
    let state = daemon_state_for_test(false).await;
    let scheduler = ExpirationScheduler::start(state.clone());
    let server = NotificationServer::new(state.clone(), scheduler);
    let message = notify_header_message();
    let header = message.header();
    let mut stream = control_signal_stream(&state, "NotificationAdded").await;

    let id = server
        .ingest_notify(
            "app".to_string(),
            0,
            String::new(),
            "summary".to_string(),
            "body".to_string(),
            Vec::new(),
            HashMap::new(),
            &header,
            0,
        )
        .await
        .expect("notify should store");

    let signal = next_signal(&mut stream).await;
    let (signal_id, show_popup) = signal
        .body()
        .deserialize::<(u32, bool)>()
        .expect("notification added body");
    assert_eq!(signal_id, id);
    assert!(show_popup);
}

#[tokio::test]
async fn ingest_notify_emits_control_close_for_evicted_active_notification() {
    let mut config = Config::default();
    config.history.max_active = 1;
    let state = daemon_state_with_config(config).await;
    let scheduler = ExpirationScheduler::start(state.clone());
    let server = NotificationServer::new(state.clone(), scheduler);
    let message = notify_header_message();
    let header = message.header();
    let mut stream = control_signal_stream(&state, "NotificationClosed").await;

    let first_id = server
        .ingest_notify(
            "app".to_string(),
            0,
            String::new(),
            "first".to_string(),
            "body".to_string(),
            Vec::new(),
            HashMap::new(),
            &header,
            0,
        )
        .await
        .expect("first notify should store");
    server
        .ingest_notify(
            "app".to_string(),
            0,
            String::new(),
            "second".to_string(),
            "body".to_string(),
            Vec::new(),
            HashMap::new(),
            &header,
            0,
        )
        .await
        .expect("second notify should store");

    let signal = next_signal(&mut stream).await;
    let (signal_id, reason) = signal
        .body()
        .deserialize::<(u32, CloseReason)>()
        .expect("notification closed body");
    assert_eq!(signal_id, first_id);
    assert_eq!(reason as u32, CloseReason::Undefined as u32);
}
