use unixnotis_core::{
    Action, AttributionReason, CloseReason, IdentityAssurance, InlineReply, InlineReplyPolicy,
    InteractionPolicies, NotificationAttribution,
};

use crate::store::test_support::{make_notification, make_store_with_limits};

#[test]
fn active_inline_reply_target_requires_a_live_explicit_reply_action() {
    let mut store = make_store_with_limits(12, 20);
    let ordinary = store
        .insert(make_notification("ordinary"), 0)
        .active_notification();
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
    let reply = store.insert(reply, 0).active_notification();

    assert!(store
        .active_inline_reply_target(ordinary.id, ordinary.generation)
        .is_none());
    let target = store
        .active_inline_reply_target(reply.id, reply.generation)
        .expect("reply target");
    assert_eq!(target.id, reply.id);
    assert!(!target.is_resident);
    assert!(store
        .active_inline_reply_target(reply.id, reply.generation.saturating_sub(1))
        .is_none());
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
    let reply = store.insert(reply, 0).active_notification();

    assert!(
        store
            .active_inline_reply_target(reply.id, reply.generation)
            .expect("resident reply target")
            .is_resident
    );

    let key = reply.key();
    assert!(
        store
            .active_action_target_generation(key, "inline-reply", true)
            .is_none(),
        "inline-reply action must be rejected through action dispatch even when confirmed"
    );

    store.close(reply.id, CloseReason::Expired);

    assert!(store
        .active_inline_reply_target(reply.id, reply.generation)
        .is_none());
    assert!(store.list_history().iter().any(|view| view.id == reply.id));
}

#[test]
fn inline_reply_metadata_without_the_protocol_action_is_rejected() {
    let mut store = make_store_with_limits(12, 20);
    let mut malformed = make_notification("metadata only");
    malformed.inline_reply.available = true;
    let malformed = store.insert(malformed, 0).active_notification();

    assert!(store
        .active_inline_reply_target(malformed.id, malformed.generation)
        .is_none());
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
    let notification = store.insert(notification, 0).active_notification();

    assert!(store
        .active_inline_reply_target(notification.id, notification.generation)
        .is_none());
}

#[test]
fn native_association_denies_reply_even_if_protocol_metadata_claims_allow() {
    let mut store = make_store_with_limits(12, 20);
    let mut notification = make_notification("native associated reply");
    notification.attribution = NotificationAttribution::associated(
        "Example Chat",
        "Example Chat",
        "org.example.Chat",
        "org.example.Chat",
        IdentityAssurance::SystemAssociated,
        InteractionPolicies::NATIVE_COMPATIBILITY,
        AttributionReason::ExactSystemExecutable,
        "protected executable association",
        "associated:system-app:org.example.Chat".to_string(),
    );
    notification.inline_reply.available = true;
    notification.inline_reply_policy = InlineReplyPolicy::Allow;
    notification.actions.push(Action {
        key: "inline-reply".to_string(),
        label: "Reply".to_string(),
    });
    let notification = store.insert(notification, 0).active_notification();

    assert!(
        store
            .active_inline_reply_target(notification.id, notification.generation)
            .is_none(),
        "native executable association cannot authorize credential-like reply input"
    );
}
