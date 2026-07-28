//! Thumbnail source decisions for notification rows

use unixnotis_core::NotificationView;
use unixnotis_ui::presentation::{NotificationPresentation, ThumbnailKind};

pub(super) fn notification_has_thumbnail(notification: &NotificationView) -> bool {
    NotificationPresentation::from_view(notification)
        .media
        .thumbnail
        == ThumbnailKind::Content
}
