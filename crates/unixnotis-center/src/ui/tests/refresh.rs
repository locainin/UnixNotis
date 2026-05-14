use super::update_next_delay;
use std::time::Duration;

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
