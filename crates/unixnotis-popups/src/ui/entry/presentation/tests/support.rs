use unixnotis_core::{
    AttributionReason, InlineReply, InlineReplyPolicy, NotificationAttribution, NotificationImage,
    NotificationView,
};

pub(super) fn notification() -> NotificationView {
    NotificationView {
        id: 7,
        generation: 11,
        app_name: "Example".to_string(),
        attribution: NotificationAttribution::verified(
            "Example",
            "Example",
            "org.example.App",
            "example-app",
            AttributionReason::ExactSystemExecutable,
            "exact system executable",
            "system-app:org.example.App".to_string(),
        ),
        summary: "Primary title".to_string(),
        body: "Supporting body".to_string(),
        actions: Vec::new(),
        inline_reply: InlineReply::default(),
        inline_reply_policy: InlineReplyPolicy::Allow,
        urgency: 1,
        category: String::new(),
        is_transient: false,
        received_at_unix_seconds: 1_000,
        image: NotificationImage::default(),
        popup_decision: unixnotis_core::PopupDecisionRecord::default(),
        popup_hide_after_ms: 0,
    }
}
