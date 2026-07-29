use unixnotis_core::{
    AttributionClass, InlineReply, InlineReplyPolicy, NotificationAttribution, NotificationImage,
    NotificationView,
};

pub(super) fn notification() -> NotificationView {
    NotificationView {
        id: 7,
        generation: 11,
        app_name: "Example".to_string(),
        attribution: NotificationAttribution::associated(
            "Example",
            "org.example.App",
            "org.example.App",
            "",
            AttributionClass::SystemAssociated,
            false,
            "system-desktop:org.example.App".to_string(),
        ),
        summary: "New message".to_string(),
        body: "Are you coming?".to_string(),
        actions: Vec::new(),
        inline_reply: InlineReply::default(),
        inline_reply_policy: InlineReplyPolicy::Allow,
        urgency: 1,
        category: String::new(),
        is_transient: false,
        received_at_unix_seconds: 1_000,
        image: NotificationImage::default(),
        popup_decision: unixnotis_core::PopupDecisionRecord::default(),
    }
}
