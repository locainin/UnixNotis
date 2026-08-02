use super::super::{ImageData, NotificationImage, NotificationVisualRole};

fn image() -> NotificationImage {
    NotificationImage {
        badge_icon: "mail".to_string(),
        sender_visual_role: NotificationVisualRole::ConversationAvatar,
        sender_visual: ImageData {
            width: 1,
            height: 1,
            rowstride: 4,
            has_alpha: true,
            bits_per_sample: 8,
            channels: 4,
            data: vec![1, 2, 3, 4],
        },
        content_image: ImageData {
            width: 1,
            height: 1,
            rowstride: 4,
            has_alpha: true,
            bits_per_sample: 8,
            channels: 4,
            data: vec![4, 3, 2, 1],
        },
    }
}

#[test]
fn listing_projection_keeps_bounded_daemon_owned_pixels() {
    let listing = image().for_listing();
    assert_eq!(listing, image());
}

#[test]
fn history_projection_keeps_safe_pixels_without_paths() {
    let history = image().for_history();
    assert_eq!(history.sender_visual.data, vec![1, 2, 3, 4]);
    assert_eq!(history.content_image.data, vec![4, 3, 2, 1]);
}
