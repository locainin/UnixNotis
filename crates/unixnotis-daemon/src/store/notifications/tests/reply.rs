use std::sync::Arc;

use unixnotis_core::{Action, CloseReason, InlineReply, InlineReplyPolicy};

use super::{make_notification, make_store_with_limits};

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

#[test]
fn generation_safe_reply_dismissal_keeps_same_id_replacement() {
    let mut store = make_store_with_limits(12, 20);
    let mut original = make_notification("original");
    original.inline_reply.available = true;
    original.actions.push(Action {
        key: "inline-reply".to_string(),
        label: "Reply".to_string(),
    });
    let original = store.insert(original, 0).notification;
    let id = original.id;

    let replacement = store.insert(make_notification("replacement"), id);
    assert!(replacement.replaced);

    assert!(!store.dismiss_active_if_current(id, &original));
    assert_eq!(
        store
            .active_notification_view(id)
            .expect("replacement should remain active")
            .summary,
        "replacement"
    );
    assert!(store.dismiss_active_if_current(id, &replacement.notification));
    assert!(store.active_notification_view(id).is_none());
}

#[test]
fn replied_generation_is_removed_after_sender_archives_it() {
    let mut store = make_store_with_limits(12, 20);
    let original = store.insert(make_notification("original"), 0).notification;
    let id = original.id;
    store.close(id, CloseReason::ClosedByCall);
    assert_eq!(store.list_history().len(), 1);

    let outcome = store.dismiss_replied_generation(id, &original);

    assert!(!outcome.removed_active);
    assert!(outcome.removed_history);
    assert!(store.list_history().is_empty());
}

#[test]
fn replied_generation_cleanup_keeps_archived_same_id_replacement() {
    let mut store = make_store_with_limits(12, 20);
    let original = store.insert(make_notification("original"), 0).notification;
    let id = original.id;
    let replacement = store.insert(make_notification("replacement"), id);
    assert!(replacement.replaced);
    store.close(id, CloseReason::ClosedByCall);

    let outcome = store.dismiss_replied_generation(id, &original);

    assert!(!outcome.removed_any());
    let history = store.list_history();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].summary, "replacement");
}
