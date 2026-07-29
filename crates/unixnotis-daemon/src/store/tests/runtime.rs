use std::sync::Arc;

use unixnotis_core::{
    Action, AttributionReason, CloseReason, Config, InlineReply, InlineReplyPolicy,
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
fn notification_diagnostics_preserve_arrival_state_after_runtime_state_changes() {
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
    let dnd_suppressed = store
        .insert(make_notification("DND suppressed"), 0)
        .notification;
    store.set_dnd(false);
    let ready = unixnotis_core::UiHealth {
        popups_process_running: true,
        popups_ready: true,
        ..unixnotis_core::UiHealth::default()
    };
    let suppressed = store
        .notification_diagnostics(dnd_suppressed.id, &ready)
        .expect("DND diagnostics");

    assert_eq!(suppressed.popup_admission, PopupAdmissionView::Dnd);
    assert!(!suppressed.renderer_process_running);
    assert!(!suppressed.renderer_ready);
}

#[test]
fn notification_diagnostics_require_both_renderer_process_and_readiness() {
    let mut store = make_store_with_limits(10, 10);
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
        let visible = store.insert(make_notification("visible"), 0).notification;
        store.record_popup_commit_environment(
            visible.key(),
            crate::store::PopupAdmission::Show,
            &health,
        );
        let diagnostics = store
            .notification_diagnostics(visible.id, &unixnotis_core::UiHealth::default())
            .expect("active notification diagnostics");

        assert_eq!(
            diagnostics.popup_admission, expected,
            "process_running={process_running}, ready={ready}"
        );
    }
}

#[test]
fn disabled_popups_are_recorded_when_max_visible_is_zero() {
    let mut config = Config::default();
    config.popups.max_visible = 0;
    let mut store = NotificationStore::new(config);
    let notification = store.insert(make_notification("disabled"), 0).notification;
    let ready = unixnotis_core::UiHealth {
        popups_process_running: true,
        popups_ready: true,
        ..unixnotis_core::UiHealth::default()
    };
    store.record_popup_commit_environment(
        notification.key(),
        crate::store::PopupAdmission::Show,
        &ready,
    );

    let diagnostics = store
        .notification_diagnostics(notification.id, &ready)
        .expect("disabled popup diagnostics");

    assert_eq!(
        diagnostics.popup_admission,
        PopupAdmissionView::RendererDisabled
    );
    assert_eq!(diagnostics.configured_max_visible, 0);
}

#[test]
fn archived_notification_keeps_its_arrival_popup_explanation() {
    let mut store = make_store_with_limits(10, 10);
    store.set_dnd(true);
    let notification = store
        .insert(make_notification("archived DND"), 0)
        .notification;
    store.close(notification.id, CloseReason::Expired);
    store.set_dnd(false);

    let diagnostics = store
        .notification_diagnostics(notification.id, &unixnotis_core::UiHealth::default())
        .expect("history diagnostics should remain available");

    assert_eq!(diagnostics.generation, notification.generation);
    assert_eq!(diagnostics.popup_admission, PopupAdmissionView::Dnd);
}

#[test]
fn popup_delivery_stage_advances_for_fetch_and_render_acknowledgement() {
    let mut store = make_store_with_limits(10, 10);
    let notification = store.insert(make_notification("delivery"), 0).notification;
    let ready = unixnotis_core::UiHealth {
        popups_process_running: true,
        popups_ready: true,
        ..unixnotis_core::UiHealth::default()
    };
    store.record_popup_commit_environment(
        notification.key(),
        crate::store::PopupAdmission::Show,
        &ready,
    );

    let candidate = store
        .popup_candidate(notification.id)
        .expect("admitted popup candidate");
    assert_eq!(candidate.admission, PopupAdmissionView::Show);
    assert_eq!(
        store
            .notification_diagnostics(notification.id, &ready)
            .expect("fetched diagnostics")
            .delivery_stage,
        unixnotis_core::PopupDeliveryStage::RendererFetched
    );

    assert_eq!(
        store.record_popup_delivery_stage(
            notification.key(),
            unixnotis_core::PopupDeliveryStage::Visible,
        ),
        crate::store::DeliveryStageUpdate::Advanced
    );
    assert_eq!(
        store
            .notification_diagnostics(notification.id, &ready)
            .expect("rendered diagnostics")
            .delivery_stage,
        unixnotis_core::PopupDeliveryStage::Visible
    );
}

#[test]
fn delivery_stage_never_moves_backward() {
    let mut store = make_store_with_limits(10, 10);
    let notification = store.insert(make_notification("delivery"), 0).notification;

    assert_eq!(
        store.record_popup_delivery_stage(
            notification.key(),
            unixnotis_core::PopupDeliveryStage::Visible,
        ),
        crate::store::DeliveryStageUpdate::Advanced
    );
    assert_eq!(
        store.record_popup_delivery_stage(
            notification.key(),
            unixnotis_core::PopupDeliveryStage::RendererFetched,
        ),
        crate::store::DeliveryStageUpdate::AlreadyAtOrBeyond
    );

    assert_eq!(
        store
            .notification_diagnostics(notification.id, &unixnotis_core::UiHealth::default())
            .expect("delivery diagnostics")
            .delivery_stage,
        unixnotis_core::PopupDeliveryStage::Visible,
        "later duplicate fetches must not regress delivery history"
    );
}

#[test]
fn duplicate_popup_stage_acknowledgement_is_idempotent() {
    let mut store = make_store_with_limits(10, 10);
    let notification = store.insert(make_notification("delivery"), 0).notification;

    assert_eq!(
        store.record_popup_delivery_stage(
            notification.key(),
            unixnotis_core::PopupDeliveryStage::Visible,
        ),
        crate::store::DeliveryStageUpdate::Advanced
    );
    assert_eq!(
        store.record_popup_delivery_stage(
            notification.key(),
            unixnotis_core::PopupDeliveryStage::Visible,
        ),
        crate::store::DeliveryStageUpdate::AlreadyAtOrBeyond,
        "a retained generation must accept a duplicate renderer callback"
    );
}

#[test]
fn popup_stage_acknowledgement_rejects_a_missing_generation() {
    let mut store = make_store_with_limits(10, 10);
    let original = store.insert(make_notification("original"), 0).notification;
    let _replacement = store
        .insert(make_notification("replacement"), original.id)
        .notification;

    assert_eq!(
        store.record_popup_delivery_stage(
            original.key(),
            unixnotis_core::PopupDeliveryStage::Visible,
        ),
        crate::store::DeliveryStageUpdate::MissingGeneration,
        "a stale generation must remain distinct from an idempotent current callback"
    );
}

#[test]
fn popup_candidate_list_requires_policy_and_arrival_decision_to_allow_rendering() {
    let mut store = make_store_with_limits(10, 10);
    let ready = unixnotis_core::UiHealth {
        popups_process_running: true,
        popups_ready: true,
        ..unixnotis_core::UiHealth::default()
    };

    let mut rule_suppressed = make_notification("persistent suppression");
    rule_suppressed.suppress_popup = true;
    let rule_suppressed = store.insert(rule_suppressed, 0).notification;
    store.record_popup_commit_environment(
        rule_suppressed.key(),
        crate::store::PopupAdmission::Show,
        &ready,
    );

    let arrival_suppressed = store
        .insert(make_notification("arrival suppression"), 0)
        .notification;
    store.record_popup_commit_environment(
        arrival_suppressed.key(),
        crate::store::PopupAdmission::Suppressed(crate::store::PopupSuppressionReason::Rule),
        &ready,
    );

    let admitted = store.insert(make_notification("admitted"), 0).notification;
    store.record_popup_commit_environment(
        admitted.key(),
        crate::store::PopupAdmission::Show,
        &ready,
    );

    let candidates = store.list_popup_candidates();
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].key(), admitted.key());
}

#[test]
fn popup_decisions_are_pruned_after_their_active_and_history_generations_are_removed() {
    let mut store = make_store_with_limits(10, 10);
    let notification = store.insert(make_notification("retained"), 0).notification;

    assert!(store.popup_decisions.contains_key(&notification.key()));
    store.close(notification.id, CloseReason::Expired);
    assert!(store.popup_decisions.contains_key(&notification.key()));

    store.clear_history();
    assert!(store.popup_decisions.is_empty());
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
    notification.attribution = NotificationAttribution::verified(
        "Action source",
        "Action source",
        "org.example.ActionSource",
        "",
        AttributionReason::ExactSystemExecutable,
        "exact system executable",
        "system-app:org.example.ActionSource".to_string(),
    );
    notification.actions.push(Action {
        key: "open".to_string(),
        label: "Open".to_string(),
    });
    let original = store.insert(notification, 0).notification;
    let id = original.id;
    let key = original.key();

    let target = store
        .active_action_target_generation(key, "open")
        .expect("stored action should resolve");
    assert!(Arc::ptr_eq(&target, &original));
    assert!(store
        .active_action_target_generation(key, "missing")
        .is_none());
    assert!(store.is_active_notification_generation(id, &original));

    let replacement = store.insert(make_notification("replacement"), id);
    assert!(replacement.replaced);
    assert!(!store.is_active_notification_generation(id, &original));
    assert!(store.active_action_target_generation(key, "open").is_none());
}

#[test]
fn active_action_target_denies_every_unverified_sender_class() {
    for attribution in [
        NotificationAttribution::recognized(
            "User application",
            "User application",
            "org.example.UserApplication",
            "",
            AttributionReason::ExactUserExecutable,
            "exact user executable",
            "user-app:org.example.UserApplication".to_string(),
        ),
        NotificationAttribution::unresolved(
            "Signal",
            AttributionReason::NoDesktopCandidate,
            "source /tmp/fake",
            "unknown:signal".to_string(),
        ),
        NotificationAttribution::conflict(
            "Signal",
            "org.signal.Signal",
            AttributionReason::ExecutableMismatch,
            "source /tmp/fake",
            "conflict:signal".to_string(),
        ),
        NotificationAttribution::relay(
            "Signal",
            "trusted relay /usr/bin/notify-send",
            "relay:notify-send:signal".to_string(),
        ),
    ] {
        let mut store = make_store_with_limits(12, 20);
        let mut notification = make_notification("untrusted action");
        notification.attribution = attribution;
        notification.actions.push(Action {
            key: "default".to_string(),
            label: "Open".to_string(),
        });
        let key = store.insert(notification, 0).notification.key();

        assert!(
            store
                .active_action_target_generation(key, "default")
                .is_none(),
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
