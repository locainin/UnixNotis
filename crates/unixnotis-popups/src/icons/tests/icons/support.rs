use std::sync::MutexGuard;

use unixnotis_core::{ImageData, NotificationImage, NotificationView};

pub(super) fn image_data(
    width: i32,
    height: i32,
    rowstride: i32,
    channels: i32,
    data: Vec<u8>,
) -> ImageData {
    ImageData {
        width,
        height,
        rowstride,
        has_alpha: channels == 4,
        bits_per_sample: 8,
        channels,
        data,
    }
}

pub(super) fn notification(app_name: &str, icon_name: &str) -> NotificationView {
    NotificationView {
        id: 1,
        app_name: app_name.to_string(),
        summary: String::new(),
        body: String::new(),
        actions: Vec::new(),
        urgency: 1,
        is_transient: false,
        image: NotificationImage {
            icon_name: icon_name.to_string(),
            ..NotificationImage::default()
        },
    }
}

pub(super) fn texture_test_lock() -> MutexGuard<'static, ()> {
    // GTK texture setup shares the same global type tables as widget tests
    crate::test_support::gtk_test_lock()
}
