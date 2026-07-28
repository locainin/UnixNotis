use unixnotis_core::{
    Action, AttributionClass, ImageData, InlineReplyPolicy, NotificationAttribution, Urgency,
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
        "sender executable differs",
        "conflict:known".to_string(),
    );
    view.actions.push(Action {
        key: "default".to_string(),
        label: "Open".to_string(),
    });

    let presentation = NotificationPresentation::from_view_at(&view, 1_000);

    assert_eq!(presentation.kind, NotificationKind::Warning);
    assert_eq!(presentation.trust.level, TrustLevel::Suspicious);
    assert_eq!(
        presentation.identity.badge,
        BadgePresentation::SuspiciousApplication
    );
    assert_eq!(
        presentation.identity.secondary_claim.as_deref(),
        Some("Claims to be Known application")
    );
    assert!(presentation.actions.primary.is_empty());
    assert!(presentation.actions.overflow.is_empty());
}

#[test]
fn shared_model_keeps_user_association_unverified_and_noninteractive() {
    let mut view = notification();
    view.attribution = NotificationAttribution::associated(
        "Local application",
        "org.example.Local",
        "org.example.Local",
        "",
        AttributionClass::UserAssociated,
        false,
        "user:local".to_string(),
    );
    view.actions.push(Action {
        key: "default".to_string(),
        label: "Open".to_string(),
    });

    let presentation = NotificationPresentation::from_view_at(&view, 1_000);

    assert_eq!(presentation.trust.level, TrustLevel::Unverified);
    assert_eq!(
        presentation.identity.badge,
        BadgePresentation::UnknownApplication
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

    view.attribution.class = AttributionClass::UserAssociated;
    let unverified = NotificationPresentation::from_view_at(&view, 1_000);
    assert_eq!(unverified.trust.reply, ReplyPresentation::Unavailable);
    assert!(!unverified.critical);

    view.attribution.class = AttributionClass::SystemAssociated;
    view.urgency = Urgency::Critical as u8;
    let critical = NotificationPresentation::from_view_at(&view, 1_000);
    assert_eq!(critical.trust.reply, ReplyPresentation::Available);
    assert!(critical.critical);

    view.urgency = (Urgency::Critical as u8).saturating_add(1);
    assert!(!NotificationPresentation::from_view_at(&view, 1_000).critical);
}
