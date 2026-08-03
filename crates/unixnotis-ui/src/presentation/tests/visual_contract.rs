use unixnotis_core::{ImageData, NotificationVisualRole};

use super::super::{NotificationKind, NotificationPresentation, SenderVisualPresentation};
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
