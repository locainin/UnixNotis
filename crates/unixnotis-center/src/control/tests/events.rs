use unixnotis_core::{NotificationImage, NotificationView};

use super::{active_notification_event, UiEvent};

fn notification(id: u32) -> NotificationView {
    NotificationView {
        id,
        app_name: "example".to_string(),
        summary: "summary".to_string(),
        body: "body".to_string(),
        actions: Vec::new(),
        urgency: 1,
        is_transient: false,
        image: NotificationImage::default(),
    }
}

#[test]
fn closed_notification_race_emits_no_stale_event() {
    assert!(active_notification_event(Vec::new(), true, true).is_none());
}

#[test]
fn fetched_payload_preserves_add_update_and_popup_semantics() {
    let added = active_notification_event(vec![notification(7)], true, true);
    let updated = active_notification_event(vec![notification(8)], false, false);

    assert!(matches!(
        added,
        Some(UiEvent::NotificationAdded(notification, true)) if notification.id == 7
    ));
    assert!(matches!(
        updated,
        Some(UiEvent::NotificationUpdated(notification, false)) if notification.id == 8
    ));
}
