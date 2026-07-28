use unixnotis_core::{NotificationImage, NotificationView};

pub(super) fn notification(app_name: &str, icon_name: &str) -> NotificationView {
    NotificationView {
        id: 1,
        generation: 1,
        app_name: app_name.to_string(),
        attribution: unixnotis_core::NotificationAttribution {
            display_name: app_name.to_string(),
            badge_icon: icon_name.to_string(),
            ..unixnotis_core::NotificationAttribution::default()
        },
        summary: String::new(),
        body: String::new(),
        actions: Vec::new(),
        inline_reply: unixnotis_core::InlineReply::default(),
        inline_reply_policy: unixnotis_core::InlineReplyPolicy::Deny,
        urgency: 1,
        is_transient: false,
        image: NotificationImage {
            icon_name: icon_name.to_string(),
            ..NotificationImage::default()
        },
    }
}
