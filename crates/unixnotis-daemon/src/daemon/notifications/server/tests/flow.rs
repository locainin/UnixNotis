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
use zbus::message::{Header, Type};
use zbus::zvariant::{OwnedValue, Value};
use zbus::{Connection, MatchRule, Message, MessageStream};

use crate::daemon::notifications::identity::{SenderMetadata, SenderMetadataStatus};
use crate::daemon::notifications::ingress::payload::{
    build_notification, NotificationInput, SenderVisualRole, WireImageRole,
};
use crate::daemon::{DaemonState, NotificationServer};
use crate::expire::ExpirationScheduler;
use crate::sound::SoundSettings;
use crate::store::{
    CommitDisposition, InsertOutcome, NotificationStore, PopupAdmission, PopupSuppressionReason,
    StableProcessIdentity, SuppressedNotification,
};
use crate::test_support::daemon_state_for_test;

impl NotificationServer {
    #[expect(
        clippy::too_many_arguments,
        reason = "the freedesktop notification method defines this wire-level argument list"
    )]
    async fn ingest_notify(
        &self,
        app_name: String,
        replaces_id: u32,
        app_icon: String,
        summary: String,
        body: String,
        actions: Vec<String>,
        hints: super::super::wire_hints::WireHints,
        header: &Header<'_>,
        expire_timeout: i32,
    ) -> zbus::fdo::Result<u32> {
        let sender = self.resolve_sender(header).await;
        let completion = self
            .ingest_notify_deferred(
                app_name,
                replaces_id,
                app_icon,
                summary,
                body,
                actions,
                hints,
                sender,
                expire_timeout,
            )
            .await?;
        if let Some(suppressed) = completion.suppressed {
            self.publish_suppressed_close(suppressed).await;
        }
        Ok(completion.id)
    }
}

#[test]
fn timed_out_sender_metadata_remains_explicitly_untrusted() {
    assert_eq!(
        super::timed_out_sender_metadata().status,
        SenderMetadataStatus::CredentialLookupTimedOut
    );
}

fn notification_with_id(id: u32) -> Arc<Notification> {
    Arc::new(Notification {
        id,
        generation: 1,
        app_name: "app".to_string(),
        app_icon: String::new(),
        attribution: unixnotis_core::NotificationAttribution::default(),
        attribution_diagnostics: unixnotis_core::AttributionDiagnostics::default(),
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
    let disposition = if dropped {
        CommitDisposition::SuppressedDropAll(SuppressedNotification {
            id,
            generation: 1,
            owner: Some(StableProcessIdentity {
                pid: 42,
                start_time: 77,
            }),
        })
    } else {
        CommitDisposition::Active(notification_with_id(id))
    };
    InsertOutcome {
        disposition,
        replaced: false,
        popup_admission: if dropped {
            PopupAdmission::Suppressed(PopupSuppressionReason::DropAllInhibitor)
        } else {
            PopupAdmission::Show
        },
        allow_sound: !dropped,
        evicted: Vec::new(),
        expiration: None,
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
    use arc_swap::ArcSwap;

    use crate::daemon::DesktopIdentityIndex;

    let connection = Connection::session().await.expect("session bus");
    let sound = SoundSettings::from_config(&config, None);
    let store = NotificationStore::new_with_state_store(config, None);
    DaemonState::new_with_store(
        connection,
        store,
        sound,
        false,
        Arc::new(ArcSwap::from_pointee(DesktopIdentityIndex::default())),
        None,
    )
}

#[tokio::test(flavor = "current_thread")]
async fn notification_server_sound_dispatch_reports_accepted_and_blocked_outcomes() {
    use std::os::unix::fs::PermissionsExt;

    use crate::system_tools::routing::use_fake_tool_bin;
    use crate::test_support::TempRoot;

    let root = TempRoot::new("notification-flow-sound");
    let player = root.join("canberra-gtk-play");
    std::fs::write(&player, "#!/bin/sh\nexit 0\n").expect("write fake sound player");
    let mut permissions = std::fs::metadata(&player)
        .expect("fake sound player metadata")
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(player, permissions).expect("make fake sound player executable");
    let _tools = use_fake_tool_bin(root.path());
    let mut config = Config::default();
    config.sound.enabled = true;
    config.sound.default_name = Some("message-new".to_string());
    let state = daemon_state_with_config(config).await;
    let scheduler = ExpirationScheduler::start(state.clone());
    let server = NotificationServer::new(state, scheduler);
    let notification = notification_with_id(9);

    assert!(server.play_sound(&notification, true));
    assert!(!server.play_sound(&notification, false));
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
fn suppressed_notification_returns_content_free_lifecycle_for_dropped_payload() {
    let outcome = insert_outcome(9, true);

    let suppressed = NotificationServer::suppressed_notification(&outcome)
        .expect("DropAll outcome should retain lifecycle identity");

    assert_eq!(suppressed.id, 9);
    assert_eq!(suppressed.generation, 1);
    assert!(matches!(
        outcome.disposition,
        CommitDisposition::SuppressedDropAll(_)
    ));
}

#[test]
fn suppressed_notification_returns_none_for_stored_payload() {
    let outcome = insert_outcome(9, false);

    let suppressed = NotificationServer::suppressed_notification(&outcome);

    assert_eq!(suppressed, None);
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

#[test]
fn conversation_avatar_wire_image_is_stored_with_the_avatar_role_and_bound() {
    // Model the validated wire object immediately before notification flow routing
    let wire_image = super::super::wire_hints::WireImageData::from_parts(
        320,
        320,
        320 * 4,
        true,
        8,
        4,
        vec![19_u8; 320 * 320 * 4],
    )
    .expect("320x320 communication image should pass wire validation");
    // The communication role must send the wire image down the sender-visual branch
    let (content_image, sender_visual_data) = super::normalize_wire_image_for_role(
        WireImageRole::ConversationAvatar,
        Some(wire_image),
        None,
    );
    let notification = build_notification(NotificationInput {
        app_name: "Messages".to_string(),
        app_icon: String::new(),
        summary: "New message".to_string(),
        body: "Hello".to_string(),
        actions: Vec::new(),
        hints: HashMap::new(),
        image_data: content_image,
        sender_visual_data,
        sender_visual: None,
        sender_visual_role: SenderVisualRole::ConversationAvatar,
        sender: SenderMetadata::default(),
        attribution: unixnotis_core::NotificationAttribution::verified(
            "Messages",
            "Messages",
            "org.example.Messages",
            "messages",
            unixnotis_core::AttributionReason::ExactSystemExecutable,
            "exact system executable",
            "verified:system-app:org.example.Messages".to_string(),
        ),
        attribution_diagnostics: unixnotis_core::AttributionDiagnostics::default(),
        inline_reply_policy: unixnotis_core::InlineReplyPolicy::Deny,
        expire_timeout: 0,
    });

    assert_eq!(
        notification.image.sender_visual_role,
        unixnotis_core::NotificationVisualRole::ConversationAvatar
    );
    assert_eq!(
        (
            notification.image.sender_visual.width,
            notification.image.sender_visual.height
        ),
        (64, 64)
    );
    assert_eq!(notification.image.sender_visual.data.len(), 64 * 64 * 4);
    assert!(notification.image.content_image.data.is_empty());
}

#[tokio::test]
async fn ingest_notify_stores_notifications_and_returns_assigned_ids() {
    let state = daemon_state_for_test(false).await;
    let scheduler = ExpirationScheduler::start(state.clone());
    let server = NotificationServer::new(state.clone(), scheduler);
    let message = notify_header_message();
    let header = message.header();
    let category = OwnedValue::try_from(Value::from("im.received")).expect("category hint");
    let hints = HashMap::from([("category".to_string(), category)]);

    let id = server
        .ingest_notify(
            "app".to_string(),
            0,
            String::new(),
            "summary".to_string(),
            "body".to_string(),
            Vec::new(),
            hints.into(),
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
            HashMap::new().into(),
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
    assert_eq!(active.category, "im.received");
}

#[tokio::test]
async fn drop_all_returns_an_id_then_emits_one_content_free_close_lifecycle() {
    let mut config = Config::default();
    config.inhibit.mode = unixnotis_core::InhibitMode::DropAll;
    let state = daemon_state_with_config(config).await;
    state
        .store
        .lock()
        .await
        .add_inhibitor("test-owner".to_string(), "privacy".to_string(), 0);
    let scheduler = ExpirationScheduler::start(state.clone());
    let server = NotificationServer::new(state.clone(), scheduler);
    let message = notify_header_message();
    let header = message.header();
    let mut stream = control_signal_stream(&state, "NotificationClosed").await;

    let id = server
        .ingest_notify(
            "sensitive app".to_string(),
            0,
            String::new(),
            "secret summary".to_string(),
            "secret body".to_string(),
            Vec::new(),
            HashMap::new().into(),
            &header,
            0,
        )
        .await
        .expect("DropAll Notify should return its lifecycle ID");

    {
        let store = state.store.lock().await;
        assert!(store.list_active().is_empty());
        assert!(store.list_history().is_empty());
    }
    let signal = next_signal(&mut stream).await;
    let (closed_id, generation, reason) = signal
        .body()
        .deserialize::<(u32, u64, CloseReason)>()
        .expect("content-free close body");
    assert_eq!(closed_id, id);
    assert_ne!(generation, 0);
    assert_eq!(reason, CloseReason::Undefined);
}

#[tokio::test]
async fn ingest_notify_schedules_expiration_for_positive_transient_timeout() {
    let state = daemon_state_for_test(false).await;
    let scheduler = ExpirationScheduler::start(state.clone());
    let server = NotificationServer::new(state.clone(), scheduler);
    let message = notify_header_message();
    let header = message.header();
    let mut hints = HashMap::new();
    hints.insert("transient".to_string(), OwnedValue::from(true));

    let id = server
        .ingest_notify(
            "app".to_string(),
            0,
            String::new(),
            "expires".to_string(),
            "body".to_string(),
            Vec::new(),
            hints.into(),
            &header,
            25,
        )
        .await
        .expect("notify should store");

    let view = state
        .store
        .lock()
        .await
        .active_notification_view(id)
        .expect("positive-timeout notification should be active initially");
    assert_eq!(view.popup_hide_after_ms, 25);

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
async fn ingest_notify_expires_ordinary_positive_timeout() {
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
            "persistent action".to_string(),
            "body".to_string(),
            vec!["default".to_string(), "View".to_string()],
            HashMap::new().into(),
            &header,
            25,
        )
        .await
        .expect("notify should store");

    for _ in 0..30 {
        let store = state.store.lock().await;
        if store.active_notification_view(id).is_none() {
            let history = store.list_history();
            let archived = history
                .iter()
                .find(|notification| notification.id == id)
                .expect("expired notification should be archived");
            assert_eq!(archived.popup_hide_after_ms, 0);
            assert_eq!(archived.actions.len(), 1);
            return;
        }
        drop(store);
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    panic!("ordinary positive timeout should expire the active record");
}

#[tokio::test]
async fn default_popup_display_timeout_does_not_archive_the_active_notification() {
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
            "keeps actions live".to_string(),
            "body".to_string(),
            vec!["default".to_string(), "View".to_string()],
            HashMap::new().into(),
            &header,
            -1,
        )
        .await
        .expect("notify should store");

    tokio::time::sleep(Duration::from_millis(40)).await;
    let store = state.store.lock().await;
    let active = store
        .active_notification_view(id)
        .expect("default popup timeout must not close active storage");
    assert_eq!(active.summary, "keeps actions live");
    assert_eq!(active.actions.len(), 1);
    assert_eq!(
        active.popup_hide_after_ms,
        Config::default().popups.default_timeout_ms
    );
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
            HashMap::new().into(),
            &header,
            0,
        )
        .await
        .expect("notify should store");

    let signal = next_signal(&mut stream).await;
    let (signal_id, generation) = signal
        .body()
        .deserialize::<(u32, u64)>()
        .expect("notification added body");
    assert_eq!(signal_id, id);
    assert_eq!(
        generation,
        state
            .store
            .lock()
            .await
            .active_notification_view(id)
            .expect("signalled notification should remain active")
            .generation
    );
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
            HashMap::new().into(),
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
            HashMap::new().into(),
            &header,
            0,
        )
        .await
        .expect("second notify should store");

    let signal = next_signal(&mut stream).await;
    let (signal_id, signal_generation, reason) = signal
        .body()
        .deserialize::<(u32, u64, CloseReason)>()
        .expect("notification closed body");
    assert_eq!(signal_id, first_id);
    assert!(signal_generation > 0);
    assert_eq!(reason as u32, CloseReason::Undefined as u32);
}
