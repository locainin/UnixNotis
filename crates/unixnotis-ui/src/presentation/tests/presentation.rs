use unixnotis_core::{
    Action, AttributionReason, AttributionStatus, ImageData, InlineReplyPolicy,
    NotificationAttribution, Urgency,
};

use super::super::{
    BadgePresentation, NotificationKind, NotificationPresentation, ReplyPresentation,
    ThumbnailKind, TrustLevel,
};
use super::support::notification;

#[test]
fn shared_model_keeps_verified_communication_content_and_actions_consistent() {
    let mut view = notification();
    view.category = "im.received".to_string();
    view.inline_reply.available = true;
    view.actions = vec![
        Action {
            key: "inline-reply".to_string(),
            label: "Reply".to_string(),
        },
        Action {
            key: "default".to_string(),
            label: "Open".to_string(),
        },
    ];
    view.image.has_image_data = true;
    view.image.image_data = ImageData {
        width: 64,
        height: 64,
        ..ImageData::default()
    };

    let presentation = NotificationPresentation::from_view_at(&view, 1_120);

    assert_eq!(presentation.kind, NotificationKind::Communication);
    assert_eq!(presentation.trust.level, TrustLevel::Verified);
    assert_eq!(presentation.trust.reply, ReplyPresentation::Available);
    assert_eq!(
        presentation.identity.badge,
        BadgePresentation::AuthenticatedApplication
    );
    assert_eq!(presentation.media.thumbnail, ThumbnailKind::Content);
    assert_eq!(presentation.actions.primary.len(), 1);
    assert!(presentation.actions.overflow.is_empty());
    assert_eq!(presentation.timestamp, "2m");
}

#[test]
fn shared_model_downgrades_conflicts_and_denies_application_interaction() {
    let mut view = notification();
    view.attribution = NotificationAttribution::conflict(
        "Known application",
        "org.example.Known",
        AttributionReason::ExecutableMismatch,
        "sender executable differs",
        "conflict:known".to_string(),
    );
    view.actions.push(Action {
        key: "default".to_string(),
        label: "Open".to_string(),
    });

    let presentation = NotificationPresentation::from_view_at(&view, 1_000);

    assert_eq!(presentation.kind, NotificationKind::Utility);
    assert_eq!(presentation.trust.level, TrustLevel::Conflict);
    assert_eq!(
        presentation.identity.badge,
        BadgePresentation::SuspiciousApplication
    );
    assert_eq!(
        presentation.identity.secondary_claim.as_deref(),
        Some("Claimed app: Known application")
    );
    assert!(presentation.actions.primary.is_empty());
    assert!(presentation.actions.overflow.is_empty());
}

#[test]
fn trusted_relay_claim_never_becomes_the_primary_application_identity() {
    let mut view = notification();
    view.category = "im.received".to_string();
    view.attribution = NotificationAttribution::relay(
        "Signal",
        "Sent via /usr/bin/notify-send",
        "relay:notify-send:signal".to_string(),
    );
    view.image.icon_name = "signal-desktop".to_string();

    let presentation = NotificationPresentation::from_view_at(&view, 1_000);

    assert_eq!(presentation.kind, NotificationKind::Communication);
    assert_eq!(presentation.trust.level, TrustLevel::Relay);
    assert!(presentation.trust.short_label.is_none());
    assert_eq!(
        presentation.identity.primary_label,
        "Command-line notification"
    );
    assert_eq!(
        presentation.identity.secondary_claim.as_deref(),
        Some("App label: Signal")
    );
    assert_eq!(presentation.identity.badge, BadgePresentation::CommandLine);
    assert_eq!(presentation.media.thumbnail, ThumbnailKind::None);
}

#[test]
fn unknown_claim_stays_secondary_and_unverified() {
    let mut view = notification();
    view.attribution = NotificationAttribution::unresolved(
        "Local helper",
        AttributionReason::NoDesktopCandidate,
        "Source: /tmp/local-helper",
        "unknown:local-helper".to_string(),
    );

    let presentation = NotificationPresentation::from_view_at(&view, 1_000);

    assert_eq!(presentation.trust.level, TrustLevel::Unresolved);
    assert_eq!(presentation.identity.primary_label, "Unknown application");
    assert_eq!(
        presentation.identity.secondary_claim.as_deref(),
        Some("App label: Local helper")
    );
}

#[test]
fn communication_layout_is_preserved_for_unverified_sender() {
    let mut view = notification();
    view.category = "im.received".to_string();
    view.attribution = NotificationAttribution::unresolved(
        "Local chat",
        AttributionReason::MissingSenderEvidence,
        "sender evidence unavailable",
        "unknown:local-chat".to_string(),
    );

    let presentation = NotificationPresentation::from_view_at(&view, 1_000);

    assert_eq!(
        presentation.kind,
        NotificationKind::Communication,
        "attribution uncertainty must not erase message semantics"
    );
    assert_eq!(presentation.trust.level, TrustLevel::Unresolved);
}

#[test]
fn untrusted_non_media_notification_cannot_render_content_art() {
    let mut view = notification();
    view.attribution = NotificationAttribution::relay(
        "Signal",
        "Sent via /usr/bin/notify-send",
        "relay:notify-send:signal".to_string(),
    );
    view.image.image_path = "/tmp/signal-logo.png".to_string();

    assert_eq!(
        NotificationPresentation::from_view_at(&view, 1_000)
            .media
            .thumbnail,
        ThumbnailKind::None
    );

    view.category = "image.received".to_string();
    assert_eq!(
        NotificationPresentation::from_view_at(&view, 1_000)
            .media
            .thumbnail,
        ThumbnailKind::Content
    );
}

#[test]
fn popup_status_uses_the_committed_reason_instead_of_current_state() {
    let mut view = notification();
    view.popup_decision = unixnotis_core::PopupDecisionRecord {
        admission_at_commit: unixnotis_core::PopupAdmissionView::RendererDisabled,
        renderer_process_running_at_commit: true,
        renderer_ready_at_commit: true,
        max_visible_at_commit: 0,
        decided_at_unix_ms: 1_000,
        delivery_stage: unixnotis_core::PopupDeliveryStage::Suppressed,
    };

    assert_eq!(
        NotificationPresentation::from_view_at(&view, 1_000)
            .popup_status
            .as_deref(),
        Some("Not shown — popups are disabled")
    );
}

#[test]
fn popup_status_distinguishes_renderer_recovery_and_delivery_failure() {
    for (stage, admission, expected) in [
        (
            unixnotis_core::PopupDeliveryStage::Visible,
            unixnotis_core::PopupAdmissionView::RendererUnavailable,
            None,
        ),
        (
            unixnotis_core::PopupDeliveryStage::RendererFetched,
            unixnotis_core::PopupAdmissionView::RendererUnavailable,
            Some("Not shown — popup renderer was unavailable"),
        ),
        (
            unixnotis_core::PopupDeliveryStage::FanoutFailed,
            unixnotis_core::PopupAdmissionView::Show,
            Some("Not shown — live notification delivery failed"),
        ),
        (
            unixnotis_core::PopupDeliveryStage::Visible,
            unixnotis_core::PopupAdmissionView::Show,
            None,
        ),
    ] {
        let mut view = notification();
        view.popup_decision = unixnotis_core::PopupDecisionRecord {
            admission_at_commit: admission,
            decided_at_unix_ms: 1_000,
            delivery_stage: stage,
            ..unixnotis_core::PopupDecisionRecord::default()
        };

        assert_eq!(
            NotificationPresentation::from_view_at(&view, 1_000)
                .popup_status
                .as_deref(),
            expected,
            "stage={stage:?}, admission={admission:?}"
        );
    }
}

#[test]
fn suppression_reason_survives_a_later_fanout_failure() {
    for (admission, expected) in [
        (
            unixnotis_core::PopupAdmissionView::Dnd,
            "Not shown — Do Not Disturb was enabled",
        ),
        (
            unixnotis_core::PopupAdmissionView::Rule,
            "Not shown — matched a notification rule",
        ),
        (
            unixnotis_core::PopupAdmissionView::Inhibitor,
            "Not shown — notifications were inhibited",
        ),
    ] {
        let mut view = notification();
        view.popup_decision = unixnotis_core::PopupDecisionRecord {
            admission_at_commit: admission,
            decided_at_unix_ms: 1_000,
            delivery_stage: unixnotis_core::PopupDeliveryStage::FanoutFailed,
            ..unixnotis_core::PopupDecisionRecord::default()
        };

        assert_eq!(
            NotificationPresentation::from_view_at(&view, 1_000)
                .popup_status
                .as_deref(),
            Some(expected),
            "the arrival decision must outrank later delivery state"
        );
    }
}

#[test]
fn empty_and_generic_claims_never_create_secondary_identity_copy() {
    for claim in ["", "Unknown application"] {
        let mut view = notification();
        view.attribution = NotificationAttribution::relay(
            claim,
            "Sent via /usr/bin/notify-send",
            format!("relay:notify-send:{claim}"),
        );

        assert!(
            NotificationPresentation::from_view_at(&view, 1_000)
                .identity
                .secondary_claim
                .is_none(),
            "claim={claim:?}"
        );
    }
}

#[test]
fn verified_media_category_or_pixel_data_can_override_duplicate_badge_suppression() {
    for (has_image_data, category) in [(false, "image.received"), (true, "")] {
        let mut view = notification();
        view.attribution.badge_icon = "same-icon".to_string();
        view.image.image_path = "same-icon".to_string();
        view.image.has_image_data = has_image_data;
        view.category = category.to_string();

        assert_eq!(
            NotificationPresentation::from_view_at(&view, 1_000)
                .media
                .thumbnail,
            ThumbnailKind::Content,
            "has_image_data={has_image_data}, category={category:?}"
        );
    }
}

#[test]
fn shared_model_keeps_user_association_unverified_and_noninteractive() {
    let mut view = notification();
    view.attribution = NotificationAttribution::recognized(
        "Local application",
        "Local application",
        "org.example.Local",
        "org.example.Local",
        AttributionReason::ExactUserExecutable,
        "user-local desktop association",
        "user:local".to_string(),
    );
    view.actions.push(Action {
        key: "default".to_string(),
        label: "Open".to_string(),
    });

    let presentation = NotificationPresentation::from_view_at(&view, 1_000);

    assert_eq!(presentation.trust.level, TrustLevel::Recognized);
    assert_eq!(
        presentation.identity.badge,
        BadgePresentation::RecognizedApplication
    );
    assert!(presentation.actions.primary.is_empty());
}

#[test]
fn shared_model_requires_every_reply_authorization_condition() {
    let cases = [
        (false, false, InlineReplyPolicy::Deny, false),
        (true, false, InlineReplyPolicy::Allow, false),
        (false, true, InlineReplyPolicy::Allow, false),
        (true, true, InlineReplyPolicy::Deny, false),
        (true, true, InlineReplyPolicy::Allow, true),
    ];

    for (has_action, metadata_available, policy, expected_available) in cases {
        let mut view = notification();
        view.inline_reply.available = metadata_available;
        view.inline_reply_policy = policy;
        if has_action {
            view.actions.push(Action {
                key: "inline-reply".to_string(),
                label: "Reply".to_string(),
            });
        }

        let presentation = NotificationPresentation::from_view_at(&view, 1_000);
        let expected = if expected_available {
            ReplyPresentation::Available
        } else if has_action || metadata_available {
            ReplyPresentation::Unavailable
        } else {
            ReplyPresentation::Hidden
        };

        assert_eq!(
            presentation.trust.reply, expected,
            "has_action={has_action}, metadata_available={metadata_available}, policy={policy:?}"
        );
    }
}

#[test]
fn shared_model_requires_verified_identity_and_exact_critical_urgency() {
    let mut view = notification();
    view.inline_reply.available = true;
    view.actions.push(Action {
        key: "inline-reply".to_string(),
        label: "Reply".to_string(),
    });

    view.attribution.status = AttributionStatus::Recognized;
    let unverified = NotificationPresentation::from_view_at(&view, 1_000);
    assert_eq!(unverified.trust.reply, ReplyPresentation::Unavailable);
    assert!(!unverified.critical);

    view.attribution.status = AttributionStatus::Verified;
    view.urgency = Urgency::Critical as u8;
    let critical = NotificationPresentation::from_view_at(&view, 1_000);
    assert_eq!(critical.trust.reply, ReplyPresentation::Available);
    assert!(critical.critical);

    view.urgency = (Urgency::Critical as u8).saturating_add(1);
    assert!(!NotificationPresentation::from_view_at(&view, 1_000).critical);
}
