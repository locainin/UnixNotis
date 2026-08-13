use super::{
    generation_matches, incoming_generation_is_stale, popup_can_skip_rebuild,
    popup_payload_is_unchanged, VisiblePopupUpdate,
};
use unixnotis_core::{Action, NotificationImage, NotificationView};

#[test]
fn visible_update_starts_without_stack_changes() {
    let update = VisiblePopupUpdate::default();

    assert!(!update.stack_changed);
}

#[test]
fn newer_popup_generation_rejects_reordered_older_update() {
    assert!(incoming_generation_is_stale(Some(8), 7));
    assert!(!incoming_generation_is_stale(Some(8), 8));
    assert!(!incoming_generation_is_stale(Some(7), 8));
    assert!(!incoming_generation_is_stale(None, 8));
}

#[test]
fn popup_close_matches_only_the_exact_generation() {
    assert!(generation_matches(Some(8), 8));
    assert!(!generation_matches(Some(8), 7));
    assert!(!generation_matches(None, 8));
}

#[test]
fn identical_same_generation_payloads_do_not_need_a_row_rebuild() {
    let notification = NotificationView {
        id: 7,
        generation: 3,
        app_name: "Test".to_string(),
        attribution: unixnotis_core::NotificationAttribution::default(),
        summary: "Summary".to_string(),
        body: "Body".to_string(),
        actions: vec![Action {
            key: "open".to_string(),
            label: "Open".to_string(),
        }],
        inline_reply: unixnotis_core::InlineReply::default(),
        inline_reply_policy: unixnotis_core::InlineReplyPolicy::Allow,
        urgency: 1,
        category: String::new(),
        is_transient: false,
        received_at_unix_seconds: 0,
        image: NotificationImage::default(),
        popup_decision: unixnotis_core::PopupDecisionRecord::default(),
        popup_hide_after_ms: 0,
    };

    assert!(popup_payload_is_unchanged(
        &notification,
        &notification.clone()
    ));

    let mut changed = notification.clone();
    changed.summary = "Changed".to_string();
    assert!(!popup_payload_is_unchanged(&notification, &changed));
}

#[test]
fn identical_payloads_require_rebuild_when_icon_sources_are_stale() {
    let notification = NotificationView {
        id: 9,
        generation: 4,
        app_name: "Test".to_string(),
        attribution: unixnotis_core::NotificationAttribution::default(),
        summary: "Summary".to_string(),
        body: "Body".to_string(),
        actions: Vec::new(),
        inline_reply: unixnotis_core::InlineReply::default(),
        inline_reply_policy: unixnotis_core::InlineReplyPolicy::Allow,
        urgency: 1,
        category: String::new(),
        is_transient: false,
        received_at_unix_seconds: 0,
        image: NotificationImage::default(),
        popup_decision: unixnotis_core::PopupDecisionRecord::default(),
        popup_hide_after_ms: 0,
    };

    assert!(!popup_can_skip_rebuild(
        &notification,
        &notification,
        2,
        3,
        false,
    ));
    assert!(!popup_can_skip_rebuild(
        &notification,
        &notification,
        3,
        3,
        true,
    ));
    assert!(popup_can_skip_rebuild(
        &notification,
        &notification,
        3,
        3,
        false,
    ));
}
