//! Thumbnail source decisions for notification rows

use unixnotis_core::NotificationView;

pub(super) fn notification_has_thumbnail(notification: &NotificationView) -> bool {
    notification.image.has_image_data || !notification.image.image_path.trim().is_empty()
}
