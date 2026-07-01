use super::{interval_due, is_due_delay, update_next_delay};
use std::time::Duration;
use std::time::Instant;

#[test]
fn deadline_scheduler_supports_lower_idle_wakeups_than_fixed_ticks() {
    // Legacy slow polling wakes every 3s with defaults (20 wakeups/minute).
    let legacy_wakeups_per_min = 60.0 / 3.0;

    // Deadline model uses each widget's own due time. Stable stats at 12s and
    // calendar at daily cadence produce a 12s next wakeup (5 wakeups/minute).
    let mut next = None;
    update_next_delay(&mut next, Some(Duration::from_secs(12)));
    update_next_delay(&mut next, Some(Duration::from_secs(24 * 60 * 60)));
    let delay = next.expect("next deadline");
    let deadline_wakeups_per_min = 60.0 / delay.as_secs_f64();

    assert!(deadline_wakeups_per_min < legacy_wakeups_per_min);
    assert_eq!(delay, Duration::from_secs(12));
}

#[test]
fn update_next_delay_keeps_the_earliest_widget_deadline() {
    let mut next = None;

    // Later candidates must not push out a near-term wakeup
    update_next_delay(&mut next, Some(Duration::from_secs(10)));
    update_next_delay(&mut next, Some(Duration::from_secs(30)));
    update_next_delay(&mut next, Some(Duration::from_secs(2)));

    assert_eq!(next, Some(Duration::from_secs(2)));
}

#[test]
fn update_next_delay_ignores_disabled_widget_lanes() {
    let mut next = Some(Duration::from_secs(8));

    // None means a widget has no polling work and should not affect the timer
    update_next_delay(&mut next, None);

    assert_eq!(next, Some(Duration::from_secs(8)));
}

#[test]
fn interval_due_returns_none_when_lane_is_disabled() {
    let now = Instant::now();

    // Zero intervals are the config-level opt-out for a polling lane
    assert_eq!(interval_due(now, Some(now), 0), None);
}

#[test]
fn interval_due_runs_immediately_without_a_previous_tick() {
    let now = Instant::now();

    // First refresh should not wait a full interval after the panel opens
    assert_eq!(interval_due(now, None, 1_000), Some(Duration::ZERO));
}

#[test]
fn interval_due_saturates_when_the_previous_tick_is_old() {
    let now = Instant::now();
    let last = now - Duration::from_secs(10);

    // Long sleep or resume gaps should not underflow the remaining delay
    assert_eq!(interval_due(now, Some(last), 1_000), Some(Duration::ZERO));
}

#[test]
fn interval_due_reports_remaining_delay_before_deadline() {
    let now = Instant::now();
    let last = now - Duration::from_millis(250);

    assert_eq!(
        interval_due(now, Some(last), 1_000),
        Some(Duration::from_millis(750))
    );
}

#[test]
fn sub_millisecond_refresh_delay_counts_as_due() {
    // GLib timers are millisecond-granularity, so tiny delays should not spin
    assert!(is_due_delay(Duration::from_micros(500)));
}

#[test]
fn larger_refresh_delay_waits_for_the_next_timer() {
    assert!(!is_due_delay(Duration::from_millis(2)));
}
