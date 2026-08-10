use unixnotis_core::{
    AttributionReason, IdentityAssurance, ImageData, InteractionPolicies, NotificationAttribution,
    NotificationVisualRole,
};

use super::super::{
    NotificationKind, NotificationPresentation, SenderVisualPresentation, TrustLevel,
};
use super::support::notification;

#[test]
fn shared_notification_visual_contract_covers_client_surface_matrix() {
    let cases = [
        (
            "utility",
            NotificationKind::Utility,
            NotificationVisualRole::None,
        ),
        (
            "communication-avatar",
            NotificationKind::Communication,
            NotificationVisualRole::ConversationAvatar,
        ),
        (
            "media-content",
            NotificationKind::Media,
            NotificationVisualRole::ContentImage,
        ),
        (
            "utility-application-visual",
            NotificationKind::Utility,
            NotificationVisualRole::ApplicationProvidedIcon,
        ),
    ];

    for (name, expected_kind, role) in cases {
        let mut view = notification();
        view.category = match expected_kind {
            NotificationKind::Utility => String::new(),
            NotificationKind::Communication => "message.received".to_string(),
            NotificationKind::Media => "media.player".to_string(),
        };
        view.image.sender_visual_role = role;
        if role == NotificationVisualRole::ConversationAvatar {
            view.image.sender_visual = ImageData {
                width: 1,
                height: 1,
                rowstride: 4,
                bits_per_sample: 8,
                channels: 4,
                data: vec![1, 2, 3, 255],
                ..ImageData::default()
            };
        }
        if role == NotificationVisualRole::ApplicationProvidedIcon {
            view.image.sender_visual = ImageData {
                width: 1,
                height: 1,
                rowstride: 4,
                bits_per_sample: 8,
                channels: 4,
                data: vec![9, 8, 7, 255],
                ..ImageData::default()
            };
        }
        if role == NotificationVisualRole::ContentImage {
            view.image.content_image = ImageData {
                width: 1,
                height: 1,
                rowstride: 4,
                bits_per_sample: 8,
                channels: 4,
                data: vec![4, 5, 6, 255],
                ..ImageData::default()
            };
        }

        let presentation = NotificationPresentation::from_view_at(&view, 1_000);
        assert_eq!(presentation.kind, expected_kind, "case={name}");
        assert_eq!(
            presentation.visuals.sender,
            match role {
                NotificationVisualRole::ConversationAvatar => {
                    SenderVisualPresentation::ConversationAvatar
                }
                NotificationVisualRole::None | NotificationVisualRole::ContentImage => {
                    SenderVisualPresentation::None
                }
                NotificationVisualRole::ApplicationProvidedIcon => {
                    SenderVisualPresentation::ApplicationProvidedIcon
                }
            },
            "case={name}"
        );
        assert_eq!(
            presentation.visuals.content_image,
            role == NotificationVisualRole::ContentImage,
            "case={name}"
        );
    }
}

#[test]
fn conversation_pixels_keep_avatar_role_across_trust_states() {
    for (name, attribution) in conversation_attribution_cases() {
        let mut view = notification();
        view.attribution = attribution;
        view.image.sender_visual_role = NotificationVisualRole::ConversationAvatar;
        view.image.sender_visual = ImageData {
            width: 1,
            height: 1,
            rowstride: 4,
            bits_per_sample: 8,
            channels: 4,
            data: vec![1, 2, 3, 255],
            ..ImageData::default()
        };

        let presentation = NotificationPresentation::from_view_at(&view, 1_000);
        // Trust is carried by the separate trust presentation, never by the image role
        assert_eq!(
            presentation.visuals.sender,
            SenderVisualPresentation::ConversationAvatar,
            "case={name}"
        );
    }
}

#[test]
fn trust_state_only_controls_semantic_badge_precedence() {
    let semantic_first = [TrustLevel::Conflict, TrustLevel::Relay];
    let branding_first = [
        TrustLevel::Verified,
        TrustLevel::SystemAssociated,
        TrustLevel::PortalAssociated,
        TrustLevel::UserAssociated,
        TrustLevel::Unresolved,
    ];

    for level in semantic_first {
        assert!(level.semantic_badge_is_authoritative());
    }
    for level in branding_first {
        assert!(!level.semantic_badge_is_authoritative());
    }
}

fn conversation_attribution_cases() -> [(&'static str, NotificationAttribution); 7] {
    [
        (
            "authenticated",
            NotificationAttribution::verified(
                "Example",
                "Example",
                "org.example.App",
                "example-app",
                AttributionReason::ExactSystemExecutable,
                "authenticated test fixture",
                "verified:example".to_string(),
            ),
        ),
        (
            "system-associated",
            NotificationAttribution::associated(
                "Example",
                "Example",
                "org.example.App",
                "example-app",
                IdentityAssurance::SystemAssociated,
                InteractionPolicies::NATIVE_COMPATIBILITY,
                AttributionReason::ExactSystemExecutable,
                "system association",
                "associated:system:example".to_string(),
            ),
        ),
        (
            "user-associated",
            NotificationAttribution::associated(
                "Example",
                "Example",
                "org.example.App",
                "example-app",
                IdentityAssurance::UserAssociated,
                InteractionPolicies::CONFIRM_ACTIONS,
                AttributionReason::ExactUserExecutable,
                "user association",
                "associated:user:example".to_string(),
            ),
        ),
        (
            "portal-associated",
            NotificationAttribution::associated(
                "Example",
                "Example",
                "org.example.App",
                "example-app",
                IdentityAssurance::PortalAssociated,
                InteractionPolicies::CONFIRM_ACTIONS,
                AttributionReason::PortalAppIdAssociation,
                "portal association",
                "associated:portal:example".to_string(),
            ),
        ),
        (
            "unresolved",
            NotificationAttribution::unresolved(
                "Example",
                AttributionReason::MissingSenderEvidence,
                "no sender evidence",
                "unknown:example".to_string(),
            ),
        ),
        (
            "conflict",
            NotificationAttribution::conflict(
                "Example",
                "org.example.App",
                AttributionReason::ExecutableMismatch,
                "sender executable differs",
                "conflict:example".to_string(),
            ),
        ),
        (
            "relay",
            NotificationAttribution::relay(
                "Example",
                "forwarded notification",
                "relay:example".to_string(),
            ),
        ),
    ]
}
