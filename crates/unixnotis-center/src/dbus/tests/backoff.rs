use std::cell::Cell;

use super::*;

#[test]
fn backoff_resets_to_base() {
    let mut backoff = Backoff::new(10, 40);
    let first = backoff.next_sleep();
    assert!(first >= Duration::from_millis(10));

    backoff.next_sleep();
    backoff.next_sleep();
    backoff.reset();

    let reset_sleep = backoff.next_sleep();
    let max = Duration::from_millis(10 + BACKOFF_JITTER_MS);
    assert!(reset_sleep <= max);
}

#[test]
fn backoff_caps_at_max_with_jitter() {
    let mut backoff = Backoff::new(10, 40);
    for _ in 0..10 {
        let sleep = backoff.next_sleep();
        let max = Duration::from_millis(40 + BACKOFF_JITTER_MS);
        assert!(sleep <= max);
    }
}

#[test]
fn jitter_zero_returns_zero() {
    assert_eq!(jitter_duration(0), Duration::from_millis(0));
}

#[test]
fn jitter_duration_is_bounded() {
    // Ensure jitter never exceeds the configured maximum.
    let jitter = jitter_duration(5);
    assert!(jitter <= Duration::from_millis(5));
}

#[test]
fn retry_log_accepts_an_interval_larger_than_the_instant_clock_history() {
    // Extreme configuration must not panic while preparing the first warning
    let mut log = RetryLog::new(Duration::MAX);
    let debugged = Cell::new(false);

    log.log_with(|| {}, || debugged.set(true));

    assert!(debugged.get());
}

#[test]
fn retry_warning_start_time_saturates_when_the_interval_exceeds_clock_history() {
    let now = Instant::now();

    assert_eq!(instant_before_or_now(now, Duration::MAX), now);
    assert_eq!(instant_before_or_now(now, Duration::ZERO), now);
}

#[test]
fn retry_log_reset_makes_the_next_failure_visible_immediately() {
    let mut log = RetryLog::new(Duration::from_mins(1));
    let warnings = Cell::new(0usize);

    log.log_with(|| warnings.set(warnings.get() + 1), || {});
    log.log_with(|| warnings.set(warnings.get() + 1), || {});
    assert_eq!(warnings.get(), 1);

    log.reset();
    log.log_with(|| warnings.set(warnings.get() + 1), || {});
    assert_eq!(warnings.get(), 2);
}
