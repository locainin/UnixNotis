//! Metadata and relative-time rules for notification rows

use unixnotis_core::Urgency;

use super::test_support::{current_millis, sample_notification};
use super::update::{notification_meta_label, relative_time_badge};

#[test]
fn notification_metadata_falls_back_to_urgency_label() {
    let mut notification = sample_notification();
    notification.urgency = Urgency::Critical as u8;

    assert_eq!(notification_meta_label(&notification), "ALERT");
}

#[test]
fn notification_metadata_labels_cover_low_and_normal_urgency() {
    let mut notification = sample_notification();
    notification.urgency = Urgency::Low as u8;
    assert_eq!(notification_meta_label(&notification), "LOW");

    notification.urgency = Urgency::Normal as u8;
    assert_eq!(notification_meta_label(&notification), "NOTICE");
}

#[test]
fn empty_timestamp_hides_relative_time_badge() {
    assert!(relative_time_badge(0).is_empty());
}

#[test]
fn relative_time_badge_formats_minutes_hours_and_days() {
    let now = current_millis();

    assert_eq!(relative_time_badge(now - 30_000), "now");
    assert_eq!(relative_time_badge(now - 5 * 60_000), "5m");
    assert_eq!(relative_time_badge(now - 2 * 3_600_000), "2h");
    assert_eq!(relative_time_badge(now - 3 * 86_400_000), "3d");
}
