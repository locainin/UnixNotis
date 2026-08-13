use std::time::Duration;

use super::super::timeout::popup_display_timeout;
use unixnotis_core::{Config, NotificationImage, NotificationView, Urgency};

fn notification(timeout_ms: u64, urgency: Urgency) -> NotificationView {
    NotificationView {
        id: 1,
        generation: 1,
        app_name: "TestApp".to_string(),
        attribution: unixnotis_core::NotificationAttribution::default(),
        summary: "summary".to_string(),
        body: "body".to_string(),
        actions: Vec::new(),
        inline_reply: unixnotis_core::InlineReply::default(),
        inline_reply_policy: unixnotis_core::InlineReplyPolicy::Deny,
        urgency: urgency as u8,
        category: String::new(),
        is_transient: false,
        received_at_unix_seconds: 0,
        image: NotificationImage::default(),
        popup_decision: unixnotis_core::PopupDecisionRecord::default(),
        popup_hide_after_ms: timeout_ms,
    }
}

#[test]
fn normal_popup_uses_the_configured_display_timeout() {
    let config = Config::default();

    assert_eq!(
        popup_display_timeout(&notification(
            config.popups.default_timeout_ms,
            Urgency::Normal
        )),
        Some(Duration::from_millis(config.popups.default_timeout_ms))
    );
}

#[test]
fn critical_popup_without_a_critical_timeout_stays_visible() {
    assert_eq!(
        popup_display_timeout(&notification(0, Urgency::Critical)),
        None
    );
}

#[test]
fn critical_popup_uses_its_own_configured_timeout_when_present() {
    assert_eq!(
        popup_display_timeout(&notification(2_500, Urgency::Critical)),
        Some(Duration::from_millis(2_500))
    );
}

#[test]
fn zero_display_timeout_disables_local_hiding() {
    assert_eq!(
        popup_display_timeout(&notification(0, Urgency::Normal)),
        None
    );
}
