use std::collections::HashMap;

use chrono::Utc;
use futures_util::TryStreamExt;
use unixnotis_core::{Action, Notification, NotificationImage, Urgency};
use zbus::message::Type;
use zbus::zvariant::OwnedValue;
use zbus::{Connection, MatchRule, MessageStream};

use super::super::ControlServer;
use crate::daemon::NOTIFICATIONS_OBJECT_PATH;
use crate::test_support::daemon_state_for_test;

#[tokio::test]
async fn validated_action_emits_only_an_advertised_live_action() {
    let state = daemon_state_for_test(false).await;
    let sender = Connection::session().await.expect("sender session bus");
    let mut stream = action_signal_stream(&sender).await;
    let id = {
        let mut store = state.store.lock().await;
        store
            .insert(action_notification(&sender, "open"), 0)
            .notification
            .id
    };

    ControlServer::new(state)
        .invoke_validated_action(id, "open")
        .await
        .expect("invoke advertised action");

    assert_eq!(
        next_action_signal(&mut stream).await,
        (id, "open".to_string())
    );
}

#[tokio::test]
async fn action_signal_reaches_owner_but_not_unrelated_observer() {
    let state = daemon_state_for_test(false).await;
    let owner = Connection::session().await.expect("owner session bus");
    let observer = Connection::session().await.expect("observer session bus");
    let mut owner_stream = action_signal_stream(&owner).await;
    let mut observer_stream = action_signal_stream(&observer).await;
    let id = {
        let mut store = state.store.lock().await;
        store
            .insert(action_notification(&owner, "open"), 0)
            .notification
            .id
    };

    ControlServer::new(state)
        .invoke_validated_action(id, "open")
        .await
        .expect("invoke owner action");

    assert_eq!(
        next_action_signal(&mut owner_stream).await,
        (id, "open".to_string())
    );
    assert!(
        tokio::time::timeout(
            std::time::Duration::from_millis(100),
            observer_stream.try_next()
        )
        .await
        .is_err(),
        "unrelated observer must not receive action signal"
    );
}

#[tokio::test]
async fn validated_action_rejects_missing_and_stale_action_generations() {
    let state = daemon_state_for_test(false).await;
    let sender = Connection::session().await.expect("sender session bus");
    let id = {
        let mut store = state.store.lock().await;
        store
            .insert(action_notification(&sender, "open"), 0)
            .notification
            .id
    };
    let server = ControlServer::new(state.clone());

    server
        .invoke_validated_action(id, "missing")
        .await
        .expect_err("unadvertised action must fail");
    let replacement_state = state.clone();
    let replacement_sender = sender.clone();
    server
        .invoke_validated_action_with_pre_emit(id, "open", move || async move {
            let replacement = action_notification(&replacement_sender, "different");
            let outcome = replacement_state.store.lock().await.insert(replacement, id);
            assert!(outcome.replaced);
        })
        .await
        .expect_err("stale action generation must fail");
}

fn action_notification(sender: &Connection, key: &str) -> Notification {
    Notification {
        id: 0,
        generation: 0,
        app_name: "ActionApp".to_string(),
        app_icon: String::new(),
        attribution: unixnotis_core::NotificationAttribution::default(),
        summary: "Action".to_string(),
        body: String::new(),
        actions: vec![Action {
            key: key.to_string(),
            label: "Run".to_string(),
        }],
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
        sender_name: sender.unique_name().map(ToString::to_string),
        sender_pid: None,
        sender_start_time: None,
        sender_executable: None,
    }
}

async fn action_signal_stream(receiver: &Connection) -> MessageStream {
    let rule = MatchRule::builder()
        .msg_type(Type::Signal)
        .interface("org.freedesktop.Notifications")
        .expect("notification interface")
        .member("ActionInvoked")
        .expect("action member")
        .path(NOTIFICATIONS_OBJECT_PATH)
        .expect("notification path")
        .build();
    MessageStream::for_match_rule(rule, receiver, Some(8))
        .await
        .expect("action signal stream")
}

async fn next_action_signal(stream: &mut MessageStream) -> (u32, String) {
    let message = tokio::time::timeout(std::time::Duration::from_secs(1), stream.try_next())
        .await
        .expect("action signal timeout")
        .expect("read action signal")
        .expect("action signal stream ended");
    message.body().deserialize().expect("action signal body")
}
