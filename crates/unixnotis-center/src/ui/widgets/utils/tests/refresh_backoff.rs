use super::{RefreshBackoff, INFLIGHT_REFRESH_RECHECK};
use std::time::{Duration, Instant};

#[test]
fn refresh_backoff_resets_on_change() {
    let mut backoff = RefreshBackoff::default();
    let base = Duration::from_millis(100);
    let now = Instant::now();
    backoff.note_success(now, base, false);
    let next = now + base;
    assert!(backoff.should_refresh(next, false));
    backoff.note_success(next, base, true);
    // After a change, backoff resets but still waits the base interval
    assert!(!backoff.should_refresh(next, false));
    assert!(backoff.should_refresh(next + base, false));
}

#[test]
fn refresh_backoff_increases_on_stable() {
    let mut backoff = RefreshBackoff::default();
    let base = Duration::from_millis(100);
    let mut now = Instant::now();
    backoff.note_success(now, base, false);
    now += base;
    backoff.note_success(now, base, false);
    // After two stable updates, backoff should extend the next due time
    assert!(!backoff.should_refresh(now + base, false));
}

#[test]
fn refresh_backoff_increases_on_errors() {
    let mut backoff = RefreshBackoff::default();
    let base = Duration::from_millis(100);
    let now = Instant::now();
    backoff.note_error(now, base);
    // First error should still allow a quick retry
    assert!(backoff.should_refresh(now + base, false));
    backoff.note_error(now + base, base);
    // After repeated errors, backoff should delay retries
    assert!(!backoff.should_refresh(now + base, false));
}

#[test]
fn in_flight_recheck_stays_slower_than_short_command_polling() {
    // Async completion updates real deadlines, so rechecks should only be a safety net.
    assert!(INFLIGHT_REFRESH_RECHECK >= Duration::from_secs(1));
}
