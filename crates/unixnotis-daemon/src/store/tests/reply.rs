use unixnotis_core::{Action, CloseReason, InlineReply};

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

    assert_eq!(store.active_inline_reply_target(ordinary_id), None);
    assert_eq!(store.active_inline_reply_target(reply_id), Some(false));
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

    assert_eq!(store.active_inline_reply_target(id), Some(true));

    store.close(id, CloseReason::Expired);

    assert_eq!(store.active_inline_reply_target(id), None);
    assert!(store.list_history().iter().any(|view| view.id == id));
}

#[test]
fn inline_reply_metadata_without_the_protocol_action_is_rejected() {
    let mut store = make_store_with_limits(12, 20);
    let mut malformed = make_notification("metadata only");
    malformed.inline_reply.available = true;
    let id = store.insert(malformed, 0).notification.id;

    assert_eq!(store.active_inline_reply_target(id), None);
}
