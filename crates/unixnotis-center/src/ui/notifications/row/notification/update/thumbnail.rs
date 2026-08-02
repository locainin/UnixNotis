//! Thumbnail source decisions for notification rows

use unixnotis_core::NotificationView;
use unixnotis_ui::presentation::{NotificationPresentation, ThumbnailKind};

pub(super) fn notification_has_thumbnail(notification: &NotificationView) -> bool {
    NotificationPresentation::from_view(notification)
        .media
        .thumbnail
        == ThumbnailKind::Content
}

pub(super) const fn notification_has_conversation_avatar(notification: &NotificationView) -> bool {
    // Avatars are presentation-only raster data and never count as message content
    notification.image.has_conversation_avatar
}
