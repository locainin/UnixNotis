//! Metadata and relative-time rules for notification rows

use unixnotis_core::{NotificationMetadataConfig, Urgency};

use super::super::super::test_support::{current_millis, sample_notification};
use super::{notification_meta_label, relative_time_badge, relative_time_badge_at};

#[test]
fn notification_metadata_falls_back_to_urgency_label() {
    let mut notification = sample_notification();
    notification.urgency = Urgency::Critical as u8;
    let metadata = NotificationMetadataConfig::default();

    assert_eq!(notification_meta_label(&notification, &metadata), "ALERT");
}

#[test]
fn notification_metadata_labels_cover_low_and_normal_urgency() {
    let mut notification = sample_notification();
    let metadata = NotificationMetadataConfig::default();
    notification.urgency = Urgency::Low as u8;
    assert_eq!(notification_meta_label(&notification, &metadata), "LOW");

    notification.urgency = Urgency::Normal as u8;
    assert_eq!(notification_meta_label(&notification, &metadata), "NOTICE");
}

#[test]
fn empty_timestamp_hides_relative_time_badge() {
    assert!(relative_time_badge(0, &NotificationMetadataConfig::default()).is_empty());
}

#[test]
fn relative_time_badge_formats_minutes_hours_and_days() {
    let now = u128::try_from(current_millis()).expect("current time should be positive");
    let metadata = NotificationMetadataConfig::default();

    assert_eq!(
        relative_time_badge_at((now - 30_000) as i64, now, &metadata),
        "now"
    );
    assert_eq!(
        relative_time_badge_at((now - 5 * 60_000) as i64, now, &metadata),
        "5m"
    );
    assert_eq!(
        relative_time_badge_at((now - 2 * 3_600_000) as i64, now, &metadata),
        "2h"
    );
    assert_eq!(
        relative_time_badge_at((now - 3 * 86_400_000) as i64, now, &metadata),
        "3d"
    );
}

#[test]
fn custom_metadata_text_and_templates_replace_runtime_strings() {
    let mut notification = sample_notification();
    notification.urgency = Urgency::Critical as u8;
    let metadata = NotificationMetadataConfig {
        critical_label: "PRIORITY".to_string(),
        relative_hours: "{value} HOURS AGO".to_string(),
        ..NotificationMetadataConfig::default()
    };

    assert_eq!(
        notification_meta_label(&notification, &metadata),
        "PRIORITY"
    );
    assert_eq!(relative_time_badge_at(0, 0, &metadata), "");
    assert_eq!(
        relative_time_badge_at(1, 7_200_001, &metadata),
        "2 HOURS AGO"
    );
}
