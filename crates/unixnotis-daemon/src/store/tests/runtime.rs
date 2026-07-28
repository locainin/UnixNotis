use std::sync::Arc;

use unixnotis_core::{
    Action, AttributionClass, CloseReason, Config, InlineReply, InlineReplyPolicy,
    NotificationAttribution, PopupAdmissionView,
};

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
fn popup_candidate_pairs_rule_suppression_with_replacement_generation() {
    let mut store = make_store_with_limits(10, 10);
    let original = store.insert(make_notification("allowed"), 0).notification;
    let mut suppressed = make_notification("rule suppressed");
    suppressed.suppress_popup = true;
    let replacement = store.insert(suppressed, original.id).notification;

    let candidate = store
        .popup_candidate(original.id)
        .expect("replacement should remain an active popup candidate");

    assert_eq!(candidate.notification.generation, replacement.generation);
    assert_eq!(candidate.notification.summary, "rule suppressed");
    assert_eq!(candidate.admission, PopupAdmissionView::Rule);
}

#[test]
fn popup_candidate_pairs_dnd_suppression_with_replacement_generation() {
    let mut store = make_store_with_limits(10, 10);
    let original = store.insert(make_notification("allowed"), 0).notification;
    store.set_dnd(true);
    let replacement = store
        .insert(make_notification("dnd suppressed"), original.id)
        .notification;

    let candidate = store
        .popup_candidate(original.id)
        .expect("replacement should remain active during DND");

    assert_eq!(candidate.notification.generation, replacement.generation);
    assert_eq!(candidate.notification.summary, "dnd suppressed");
    assert_eq!(candidate.admission, PopupAdmissionView::Dnd);
}

#[test]
fn notification_diagnostics_report_renderer_and_store_admission_separately() {
    let mut store = make_store_with_limits(10, 10);
    let visible = store.insert(make_notification("visible"), 0).notification;
    let unavailable = store
        .notification_diagnostics(visible.id, &unixnotis_core::UiHealth::default())
        .expect("active notification diagnostics");

    assert_eq!(
        unavailable.popup_admission,
        PopupAdmissionView::RendererUnavailable
    );
    assert!(!unavailable.renderer_process_running);
    assert!(!unavailable.renderer_ready);

    store.set_dnd(true);
    let ready = unixnotis_core::UiHealth {
        popups_process_running: true,
        popups_ready: true,
        ..unixnotis_core::UiHealth::default()
    };
    let suppressed = store
        .notification_diagnostics(visible.id, &ready)
        .expect("DND diagnostics");

    assert_eq!(suppressed.popup_admission, PopupAdmissionView::Dnd);
    assert!(suppressed.renderer_process_running);
    assert!(suppressed.renderer_ready);
}

#[test]
fn notification_diagnostics_require_both_renderer_process_and_readiness() {
    let mut store = make_store_with_limits(10, 10);
    let visible = store.insert(make_notification("visible"), 0).notification;

    for (process_running, ready, expected) in [
        (false, false, PopupAdmissionView::RendererUnavailable),
        (true, false, PopupAdmissionView::RendererUnavailable),
        (false, true, PopupAdmissionView::RendererUnavailable),
        (true, true, PopupAdmissionView::Show),
    ] {
        let health = unixnotis_core::UiHealth {
            popups_process_running: process_running,
            popups_ready: ready,
            ..unixnotis_core::UiHealth::default()
        };
        let diagnostics = store
            .notification_diagnostics(visible.id, &health)
            .expect("active notification diagnostics");

        assert_eq!(
            diagnostics.popup_admission, expected,
            "process_running={process_running}, ready={ready}"
        );
    }
}

#[test]
fn active_inline_reply_target_requires_a_live_explicit_reply_action() {
    let mut store = make_store_with_limits(12, 20);
    let ordinary = store.insert(make_notification("ordinary"), 0).notification;
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
    let reply = store.insert(reply, 0).notification;

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
fn active_action_target_requires_an_exact_action_on_the_live_generation() {
    let mut store = make_store_with_limits(12, 20);
    let mut notification = make_notification("action");
    notification.attribution = NotificationAttribution::associated(
        "Action source",
        "org.example.ActionSource",
        "org.example.ActionSource",
        "",
        AttributionClass::SystemAssociated,
        false,
        "system-desktop:org.example.ActionSource".to_string(),
    );
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
fn active_action_target_denies_every_unverified_sender_class() {
    for attribution in [
        NotificationAttribution::associated(
            "User application",
            "org.example.UserApplication",
            "org.example.UserApplication",
            "",
            AttributionClass::UserAssociated,
            false,
            "user-desktop:org.example.UserApplication".to_string(),
        ),
        NotificationAttribution::unknown(
            "Signal",
            "source /tmp/fake",
            "unknown:signal".to_string(),
        ),
        NotificationAttribution::conflict(
            "Signal",
            "source /tmp/fake",
            "conflict:signal".to_string(),
        ),
    ] {
        let mut store = make_store_with_limits(12, 20);
        let mut notification = make_notification("untrusted action");
        notification.attribution = attribution;
        notification.actions.push(Action {
            key: "default".to_string(),
            label: "Open".to_string(),
        });
        let id = store.insert(notification, 0).notification.id;

        assert!(
            store.active_action_target(id, "default").is_none(),
            "weak attribution should not expose application actions"
        );
    }
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
    let reply = store.insert(reply, 0).notification;

    assert!(
        store
            .active_inline_reply_target(reply.id, reply.generation)
            .expect("resident reply target")
            .is_resident
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
    let malformed = store.insert(malformed, 0).notification;

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
    let notification = store.insert(notification, 0).notification;

    assert!(store
        .active_inline_reply_target(notification.id, notification.generation)
        .is_none());
}
