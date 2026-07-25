use unixnotis_core::{NotificationImage, NotificationView};

pub(super) fn notification(app_name: &str, icon_name: &str) -> NotificationView {
    NotificationView {
        id: 1,
        app_name: app_name.to_string(),
        attribution: unixnotis_core::NotificationAttribution {
            verified: true,
            reported_name: String::new(),
            badge_icon: icon_name.to_string(),
        },
        summary: String::new(),
        body: String::new(),
        actions: Vec::new(),
        inline_reply: unixnotis_core::InlineReply::default(),
        urgency: 1,
        is_transient: false,
        image: NotificationImage {
            icon_name: icon_name.to_string(),
            ..NotificationImage::default()
        },
    }
}
