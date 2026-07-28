use unixnotis_core::{NotificationImage, NotificationView};

use super::{active_notification_event, UiEvent};

fn notification(id: u32) -> NotificationView {
    NotificationView {
        id,
        generation: u64::from(id),
        app_name: "example".to_string(),
        attribution: unixnotis_core::NotificationAttribution::default(),
        summary: "summary".to_string(),
        body: "body".to_string(),
        actions: Vec::new(),
        inline_reply: unixnotis_core::InlineReply::default(),
        inline_reply_policy: unixnotis_core::InlineReplyPolicy::Allow,
        urgency: 1,
        category: String::new(),
        is_transient: false,
        image: NotificationImage::default(),
    }
}

#[test]
fn closed_notification_race_emits_no_stale_event() {
    assert!(active_notification_event(Vec::new(), 1, true).is_none());
}

#[test]
fn fetched_payload_preserves_matching_add_and_update_generations() {
    let added = active_notification_event(vec![notification(7)], 7, true);
    let updated = active_notification_event(vec![notification(8)], 8, false);

    assert!(matches!(
        added,
        Some(UiEvent::NotificationAdded(notification)) if notification.id == 7
    ));
    assert!(matches!(
        updated,
        Some(UiEvent::NotificationUpdated(notification)) if notification.id == 8
    ));
}

#[test]
fn fetched_replacement_is_rejected_for_older_signal_generation() {
    assert!(active_notification_event(vec![notification(8)], 7, false).is_none());
}
