//! Small notification-row tests that do not need full GTK setup
//!
//! Keeping these beside the row module makes the text rules easier to maintain

use super::state::MAX_SUMMARY_LABEL_CHARS;
use unixnotis_core::{NotificationImage, NotificationView, Urgency};

use super::update::{
    clamp_action_label_text, notification_has_thumbnail, notification_meta_label,
    optional_label_state, relative_time_badge,
};

#[test]
fn panel_summary_row_hides_when_text_is_empty() {
    // Empty summaries should not leave a blank strip above the body
    let state = optional_label_state("", MAX_SUMMARY_LABEL_CHARS);

    assert!(!state.visible);
    assert!(state.text.is_empty());
}

#[test]
fn panel_summary_row_hides_when_text_is_only_whitespace() {
    // Space-only payloads should collapse the same as truly empty payloads
    let state = optional_label_state("\n\t ", MAX_SUMMARY_LABEL_CHARS);

    assert!(!state.visible);
    assert!(state.text.is_empty());
}

#[test]
fn panel_summary_row_shows_when_text_has_real_content() {
    // Leading and trailing space should not hide actual notification text
    let state = optional_label_state("  hello  ", MAX_SUMMARY_LABEL_CHARS);

    assert!(state.visible);
    assert_eq!(state.text.as_ref(), "  hello  ");
}

#[test]
fn panel_summary_row_hides_when_clamp_intentionally_blanks_text() {
    // Zero-char clamps should collapse the row instead of leaving an empty label
    let state = optional_label_state("hello", 0);

    assert!(!state.visible);
    assert!(state.text.is_empty());
}

#[test]
fn panel_action_labels_are_clamped_before_button_build() {
    // Long labels should be shortened before the action row sees them
    let long_label = "This action label is much longer than the row should allow";
    let rendered = clamp_action_label_text(long_label);

    assert!(rendered.len() < long_label.len());
    assert!(rendered.ends_with('…'));
}

#[test]
fn notification_metadata_falls_back_to_urgency_label() {
    let mut notification = sample_notification();
    notification.urgency = Urgency::Critical as u8;

    assert_eq!(notification_meta_label(&notification), "ALERT");
}

#[test]
fn notification_thumbnail_only_uses_real_image_sources() {
    let mut notification = sample_notification();
    assert!(!notification_has_thumbnail(&notification));

    notification.image.image_path = "/tmp/demo.png".to_string();
    assert!(notification_has_thumbnail(&notification));
}

#[test]
fn empty_timestamp_hides_relative_time_badge() {
    assert!(relative_time_badge(0).is_empty());
}

fn sample_notification() -> NotificationView {
    NotificationView {
        id: 1,
        app_name: "demo".to_string(),
        summary: "summary".to_string(),
        body: "body".to_string(),
        actions: Vec::new(),
        urgency: Urgency::Normal as u8,
        is_transient: false,
        image: NotificationImage::default(),
    }
}
