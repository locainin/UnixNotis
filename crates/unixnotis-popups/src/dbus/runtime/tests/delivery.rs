use unixnotis_core::{NotificationImage, NotificationView, PopupAdmissionView, PopupCandidate};

use super::super::delivery::popup_event;
use crate::dbus::UiEvent;

fn candidate(generation: u64, admission: PopupAdmissionView) -> PopupCandidate {
    PopupCandidate {
        notification: NotificationView {
            id: 7,
            generation,
            app_name: "example".to_string(),
            attribution: unixnotis_core::NotificationAttribution::default(),
            summary: format!("generation {generation}"),
            body: String::new(),
            actions: Vec::new(),
            inline_reply: unixnotis_core::InlineReply::default(),
            inline_reply_policy: unixnotis_core::InlineReplyPolicy::Deny,
            urgency: 1,
            category: String::new(),
            is_transient: false,
            received_at_unix_seconds: 0,
            image: NotificationImage::default(),
        },
        admission,
    }
}

#[test]
fn old_allowed_signal_cannot_display_new_suppressed_replacement() {
    let event = popup_event(vec![candidate(2, PopupAdmissionView::Rule)], 1, true);

    assert!(event.is_none());
}

#[test]
fn current_suppressed_replacement_is_delivered_as_hidden_update() {
    let event = popup_event(vec![candidate(2, PopupAdmissionView::Dnd)], 2, false);

    assert!(matches!(
        event,
        Some(UiEvent::NotificationUpdated(notification, false))
            if notification.generation == 2
    ));
}

#[test]
fn missing_candidate_after_reordered_close_emits_no_event() {
    assert!(popup_event(Vec::new(), 3, false).is_none());
}
