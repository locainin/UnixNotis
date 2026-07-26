use std::sync::Arc;

use unixnotis_core::{Action, CloseReason, Config, InlineReply, InlineReplyPolicy};

use crate::store::test_support::{make_notification, make_store_with_limits};
use crate::store::NotificationStore;

#[test]
fn config_accessor_returns_runtime_config_snapshot() {
    let mut config = Config::default();
    config.history.max_entries = 77;
    config.history.max_active = 3;
    let store = NotificationStore::new(config);

    assert_eq!(store.config().history.max_entries, 77);
    assert_eq!(store.config().history.max_active, 3);
}

#[test]
fn active_notification_view_returns_current_active_payload() {
    let mut store = make_store_with_limits(10, 10);
    let outcome = store.insert(make_notification("visible"), 0);

    let view = store
        .active_notification_view(outcome.notification.id)
        .expect("active notification should be visible");

    assert_eq!(view.id, outcome.notification.id);
    assert_eq!(view.summary, "visible");
}

#[test]
fn active_inline_reply_target_requires_a_live_explicit_reply_action() {
    let mut store = make_store_with_limits(12, 20);
    let ordinary_id = store
        .insert(make_notification("ordinary"), 0)
        .notification
        .id;
    let mut reply = make_notification("reply");
    reply.inline_reply = InlineReply {
        available: true,
        label: "Reply".to_string(),
        ..InlineReply::default()
    };
    reply.actions.push(Action {
        key: "inline-reply".to_string(),
        label: "Reply".to_string(),
    });
    let reply_id = store.insert(reply, 0).notification.id;

    assert!(store.active_inline_reply_target(ordinary_id).is_none());
    let target = store
        .active_inline_reply_target(reply_id)
        .expect("reply target");
    assert_eq!(target.id, reply_id);
    assert!(!target.is_resident);
}

#[test]
fn active_action_target_requires_an_exact_action_on_the_live_generation() {
    let mut store = make_store_with_limits(12, 20);
    let mut notification = make_notification("action");
    notification.actions.push(Action {
        key: "open".to_string(),
        label: "Open".to_string(),
    });
    let original = store.insert(notification, 0).notification;
    let id = original.id;

    let target = store
        .active_action_target(id, "open")
        .expect("stored action should resolve");
    assert!(Arc::ptr_eq(&target, &original));
    assert!(store.active_action_target(id, "missing").is_none());
    assert!(store.is_active_notification_generation(id, &original));

    let replacement = store.insert(make_notification("replacement"), id);
    assert!(replacement.replaced);
    assert!(!store.is_active_notification_generation(id, &original));
    assert!(store.active_action_target(id, "open").is_none());
}

#[test]
fn inline_reply_target_reports_resident_state_and_rejects_history_entries() {
    let mut store = make_store_with_limits(12, 20);
    let mut reply = make_notification("resident reply");
    reply.inline_reply.available = true;
    reply.actions.push(Action {
        key: "inline-reply".to_string(),
        label: "Reply".to_string(),
    });
    reply.is_resident = true;
    let id = store.insert(reply, 0).notification.id;

    assert!(
        store
            .active_inline_reply_target(id)
            .expect("resident reply target")
            .is_resident
    );

    store.close(id, CloseReason::Expired);

    assert!(store.active_inline_reply_target(id).is_none());
    assert!(store.list_history().iter().any(|view| view.id == id));
}

#[test]
fn inline_reply_metadata_without_the_protocol_action_is_rejected() {
    let mut store = make_store_with_limits(12, 20);
    let mut malformed = make_notification("metadata only");
    malformed.inline_reply.available = true;
    let id = store.insert(malformed, 0).notification.id;

    assert!(store.active_inline_reply_target(id).is_none());
}

#[test]
fn inline_reply_policy_denies_a_complete_reply_action() {
    let mut store = make_store_with_limits(12, 20);
    let mut notification = make_notification("unassociated reply");
    notification.inline_reply.available = true;
    notification.inline_reply_policy = InlineReplyPolicy::Deny;
    notification.actions.push(Action {
        key: "inline-reply".to_string(),
        label: "Reply".to_string(),
    });
    let id = store.insert(notification, 0).notification.id;

    assert!(store.active_inline_reply_target(id).is_none());
}
