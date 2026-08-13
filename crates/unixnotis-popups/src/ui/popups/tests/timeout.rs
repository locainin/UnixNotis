use std::time::{Duration, Instant};

use super::super::timeout::popup_display_timeout;
use crate::ui::entry::PopupEntry;
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

#[test]
fn hover_pause_records_only_the_unconsumed_lifetime() {
    let started_at = Instant::now();
    let mut entry = PopupEntry::queued(notification(5_000, Urgency::Normal), 0);
    entry.prepare_hide_timer(Duration::from_secs(5), started_at);

    assert!(entry.pause_hide_timer(started_at + Duration::from_secs(2)));
    assert!(entry.hide_timer_is_paused());
    assert_eq!(entry.resume_hide_timer(), Some(Duration::from_secs(3)));
}

#[test]
fn hover_resume_uses_remaining_time_instead_of_original_timeout() {
    let started_at = Instant::now();
    let mut entry = PopupEntry::queued(notification(5_000, Urgency::Normal), 0);
    entry.prepare_hide_timer(Duration::from_secs(5), started_at);
    assert!(entry.pause_hide_timer(started_at + Duration::from_millis(4_800)));

    assert_eq!(entry.resume_hide_timer(), Some(Duration::from_millis(200)));
    assert!(!entry.hide_timer_is_paused());
    assert_eq!(
        entry.resume_hide_timer(),
        None,
        "duplicate leave must be a no-op"
    );
}

#[test]
fn repeated_enter_does_not_consume_or_replace_saved_remaining_time() {
    let started_at = Instant::now();
    let mut entry = PopupEntry::queued(notification(5_000, Urgency::Normal), 0);
    entry.prepare_hide_timer(Duration::from_secs(5), started_at);
    assert!(entry.pause_hide_timer(started_at + Duration::from_secs(1)));

    assert!(!entry.pause_hide_timer(started_at + Duration::from_secs(3)));
    assert_eq!(entry.resume_hide_timer(), Some(Duration::from_secs(4)));
}

#[test]
fn repeated_pause_resume_cycles_only_decrease_lifetime() {
    let started_at = Instant::now();
    let mut entry = PopupEntry::queued(notification(5_000, Urgency::Normal), 0);
    entry.prepare_hide_timer(Duration::from_secs(5), started_at);
    assert!(entry.pause_hide_timer(started_at + Duration::from_secs(1)));
    let first_remaining = entry.resume_hide_timer().expect("first resume duration");

    let resumed_at = started_at + Duration::from_secs(10);
    entry.prepare_hide_timer(first_remaining, resumed_at);
    assert!(entry.pause_hide_timer(resumed_at + Duration::from_secs(2)));
    let second_remaining = entry.resume_hide_timer().expect("second resume duration");

    assert_eq!(first_remaining, Duration::from_secs(4));
    assert_eq!(second_remaining, Duration::from_secs(2));
    assert!(second_remaining < first_remaining);
}

#[test]
fn pause_at_or_after_deadline_saturates_to_zero_without_panicking() {
    let started_at = Instant::now();
    let mut entry = PopupEntry::queued(notification(1, Urgency::Normal), 0);
    entry.prepare_hide_timer(Duration::from_millis(1), started_at);

    assert!(entry.pause_hide_timer(started_at + Duration::from_millis(2)));
    assert_eq!(entry.resume_hide_timer(), Some(Duration::ZERO));
}

#[test]
fn queued_and_zero_timeout_entries_have_no_timer_state() {
    let mut entry = PopupEntry::queued(notification(0, Urgency::Normal), 0);

    assert!(!entry.pause_hide_timer(Instant::now()));
    assert_eq!(entry.resume_hide_timer(), None);
}

#[test]
fn clearing_a_paused_entry_removes_all_timer_state() {
    let started_at = Instant::now();
    let mut entry = PopupEntry::queued(notification(5_000, Urgency::Normal), 0);
    entry.prepare_hide_timer(Duration::from_secs(5), started_at);
    assert!(entry.pause_hide_timer(started_at + Duration::from_secs(1)));

    entry.clear_hide_state();

    assert!(!entry.hide_timer_is_paused());
    assert_eq!(entry.resume_hide_timer(), None);
}
