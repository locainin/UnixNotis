use unixnotis_core::{
    Action, AttributionReason, AttributionStatus, IdentityAssurance, ImageData, InlineReplyPolicy,
    InteractionPolicies, NotificationAttribution, Urgency,
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
    assert!(presentation.actions.primary.is_empty());
    assert!(presentation.actions.overflow.is_empty());
    assert_eq!(presentation.actions.default_key.as_deref(), Some("default"));
    assert_eq!(presentation.timestamp, "2m");
}

#[test]
fn native_association_keeps_card_activation_and_confirms_only_extra_buttons() {
    let mut view = notification();
    view.attribution = NotificationAttribution::associated(
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
    view.inline_reply.available = true;
    view.inline_reply_policy = InlineReplyPolicy::Deny;
    view.actions = vec![
        Action {
            key: "default".to_string(),
            label: "Open conversation".to_string(),
        },
        Action {
            key: "archive".to_string(),
            label: "Archive".to_string(),
        },
        Action {
            key: "inline-reply".to_string(),
            label: "Reply".to_string(),
        },
    ];

    let presentation = NotificationPresentation::from_view_at(&view, 1_000);

    assert_eq!(presentation.trust.level, TrustLevel::SystemAssociated);
    assert_eq!(presentation.actions.default_key.as_deref(), Some("default"));
    assert_eq!(presentation.actions.primary.len(), 1);
    assert_eq!(presentation.actions.primary[0].key, "archive");
    assert_eq!(
        presentation.actions.primary[0].policy,
        unixnotis_core::ApplicationActionPolicy::Confirm
    );
    assert_eq!(presentation.trust.reply, ReplyPresentation::Unavailable);
}

#[test]
fn portal_association_exposes_confirmable_default_as_one_explicit_control() {
    let mut view = notification();
    view.attribution = NotificationAttribution::associated(
        "Example Portal App",
        "Example Portal App",
        "org.example.PortalApp",
        "org.example.PortalApp",
        IdentityAssurance::PortalAssociated,
        InteractionPolicies::CONFIRM_ACTIONS,
        AttributionReason::PortalAppIdAssociation,
        "portal app id without confinement provenance",
        "associated:portal-app:org.example.PortalApp".to_string(),
    );
    view.actions.push(Action {
        key: "default".to_string(),
        label: String::new(),
    });

    let blank = NotificationPresentation::from_view_at(&view, 1_000);
    assert!(blank.actions.default_key.is_none());
    assert_eq!(blank.actions.primary.len(), 1);
    assert_eq!(blank.actions.primary[0].key, "default");
    assert_eq!(blank.actions.primary[0].label, "Open notification");
    assert_eq!(
        blank.actions.primary[0].policy,
        unixnotis_core::ApplicationActionPolicy::Confirm
    );

    view.actions[0].label = "Open portal item".to_string();
    let labeled = NotificationPresentation::from_view_at(&view, 1_000);
    assert_eq!(labeled.actions.primary.len(), 1);
    assert_eq!(labeled.actions.primary[0].label, "Open portal item");

    view.actions.clear();
    let missing = NotificationPresentation::from_view_at(&view, 1_000);
    assert!(missing.actions.primary.is_empty());
}

#[test]
fn blank_default_action_keeps_card_activation_without_rendering_a_button() {
    let mut view = notification();
    view.actions = vec![Action {
        key: "default".to_string(),
        label: "  ".to_string(),
    }];

    let presentation = NotificationPresentation::from_view_at(&view, 1_000);

    assert_eq!(presentation.actions.default_key.as_deref(), Some("default"));
    assert!(presentation.actions.primary.is_empty());
    assert!(presentation.actions.overflow.is_empty());
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
fn associated_identity_discloses_a_different_caller_label_only() {
    let mut view = notification();
    view.attribution = NotificationAttribution::associated(
        "Example Chat",
        "Caller alias",
        "org.example.Chat",
        "org.example.Chat",
        IdentityAssurance::SystemAssociated,
        InteractionPolicies::NATIVE_COMPATIBILITY,
        AttributionReason::ExactSystemExecutable,
        "protected executable association",
        "associated:system-app:org.example.Chat".to_string(),
    );

    let differing = NotificationPresentation::from_view_at(&view, 1_000);
    assert_eq!(
        differing.identity.secondary_claim.as_deref(),
        Some("App label: Caller alias"),
        "a differing protocol label must remain visible as untrusted metadata"
    );

    view.attribution.claimed_name = "example chat".to_string();
    let matching = NotificationPresentation::from_view_at(&view, 1_000);
    assert!(
        matching.identity.secondary_claim.is_none(),
        "case-only canonical label differences should not duplicate identity text"
    );
}

#[test]
fn unresolved_claim_has_no_application_actions_or_reply() {
    let mut view = notification();
    view.attribution = NotificationAttribution::unresolved(
        "Signal",
        AttributionReason::NoDesktopCandidate,
        "sender has no positive application association",
        "unresolved:random-script:signal".to_string(),
    );
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

    let presentation = NotificationPresentation::from_view_at(&view, 1_000);

    assert_eq!(presentation.trust.level, TrustLevel::Unresolved);
    assert_eq!(presentation.trust.reply, ReplyPresentation::Unavailable);
    assert!(presentation.actions.default_key.is_none());
    assert!(presentation.actions.primary.is_empty());
    assert!(presentation.actions.overflow.is_empty());
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
fn reply_metadata_and_action_each_select_communication_layout() {
    let mut metadata = notification();
    metadata.inline_reply.available = true;
    assert_eq!(
        NotificationPresentation::from_view_at(&metadata, 1_000).kind,
        NotificationKind::Communication,
        "reply metadata should preserve message hierarchy without a category"
    );

    let mut action = notification();
    action.actions.push(Action {
        key: "inline-reply".to_string(),
        label: "Reply".to_string(),
    });
    assert_eq!(
        NotificationPresentation::from_view_at(&action, 1_000).kind,
        NotificationKind::Communication,
        "an explicit reply action should preserve message hierarchy"
    );
}

#[test]
fn media_category_selects_media_layout_without_image_content() {
    let mut view = notification();
    view.category = "media.player".to_string();

    assert_eq!(
        NotificationPresentation::from_view_at(&view, 1_000).kind,
        NotificationKind::Media,
        "media semantics must not depend on an optional thumbnail"
    );
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
        renderer_health_revision_at_commit: 0,
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
fn verified_plain_image_path_suppresses_only_duplicate_badging() {
    let mut view = notification();
    view.attribution.badge_icon = "same-icon".to_string();
    view.image.image_path = "same-icon".to_string();
    assert_eq!(
        NotificationPresentation::from_view_at(&view, 1_000)
            .media
            .thumbnail,
        ThumbnailKind::None,
        "the authenticated badge must not be repeated as content"
    );

    view.image.image_path = "different-content".to_string();
    assert_eq!(
        NotificationPresentation::from_view_at(&view, 1_000)
            .media
            .thumbnail,
        ThumbnailKind::Content,
        "a distinct explicit content path should remain visible"
    );

    view.attribution.badge_icon.clear();
    assert_eq!(
        NotificationPresentation::from_view_at(&view, 1_000)
            .media
            .thumbnail,
        ThumbnailKind::Content,
        "a missing badge cannot make explicit content look duplicated"
    );

    for (badge, image_path) in [
        ("relative-badge", "/absolute/content.png"),
        ("/absolute/badge.png", "relative-content"),
    ] {
        view.attribution.badge_icon = badge.to_string();
        view.image.image_path = image_path.to_string();
        assert_eq!(
            NotificationPresentation::from_view_at(&view, 1_000)
                .media
                .thumbnail,
            ThumbnailKind::Content,
            "mixed absolute and symbolic sources cannot establish duplicate identity"
        );
    }

    let fixture = std::fs::canonicalize("Cargo.toml").expect("resolve package manifest fixture");
    view.attribution.badge_icon = "Cargo.toml".to_string();
    view.image.image_path = fixture.to_string_lossy().into_owned();
    assert_eq!(
        NotificationPresentation::from_view_at(&view, 1_000)
            .media
            .thumbnail,
        ThumbnailKind::Content,
        "a relative badge name must not alias an absolute content path"
    );
}

#[cfg(unix)]
#[test]
fn verified_badge_symlink_is_suppressed_by_canonical_file_identity() {
    use std::os::unix::fs::symlink;

    let root = std::env::temp_dir().join(format!(
        "unixnotis-presentation-badge-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("create badge fixture directory");
    let badge = root.join("badge.svg");
    let alias = root.join("badge-alias.svg");
    std::fs::write(&badge, b"<svg/>").expect("write badge fixture");
    let _ = std::fs::remove_file(&alias);
    symlink(&badge, &alias).expect("create badge alias");

    let mut view = notification();
    view.attribution.badge_icon = badge.to_string_lossy().into_owned();
    view.image.image_path = alias.to_string_lossy().into_owned();
    let presentation = NotificationPresentation::from_view_at(&view, 1_000);

    assert_eq!(presentation.media.thumbnail, ThumbnailKind::None);
    std::fs::remove_file(&alias).expect("remove badge alias");
    std::fs::remove_file(&badge).expect("remove badge fixture");
    std::fs::remove_dir(&root).expect("remove badge fixture directory");
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

    assert_eq!(presentation.trust.level, TrustLevel::UserAssociated);
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
    view.attribution.assurance = IdentityAssurance::SystemAssociated;
    view.attribution.interactions = InteractionPolicies::NATIVE_COMPATIBILITY;
    view.inline_reply_policy = InlineReplyPolicy::Deny;
    let unverified = NotificationPresentation::from_view_at(&view, 1_000);
    assert_eq!(unverified.trust.reply, ReplyPresentation::Unavailable);
    assert!(!unverified.critical);

    view.attribution.status = AttributionStatus::Verified;
    view.attribution.assurance = IdentityAssurance::Authenticated;
    view.attribution.interactions = InteractionPolicies::AUTHENTICATED;
    view.inline_reply_policy = InlineReplyPolicy::Allow;
    view.urgency = Urgency::Critical as u8;
    let critical = NotificationPresentation::from_view_at(&view, 1_000);
    assert_eq!(critical.trust.reply, ReplyPresentation::Available);
    assert!(critical.critical);

    view.urgency = (Urgency::Critical as u8).saturating_add(1);
    assert!(!NotificationPresentation::from_view_at(&view, 1_000).critical);
}
