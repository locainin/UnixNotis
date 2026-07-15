use std::cell::Cell;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use super::*;

#[test]
fn jitter_state_advances_from_distinct_values() {
    let state = AtomicU64::new(7);
    let first = advance_jitter_state(&state, 1);
    let second = advance_jitter_state(&state, 1);

    assert_ne!(first, second);
    assert_eq!(state.load(Ordering::Relaxed), second);
}

#[test]
fn backoff_clamps_jitter_to_the_configured_maximum() {
    let mut backoff = Backoff::new(250, 1_000);

    assert_eq!(
        backoff.next_sleep_with_jitter(Duration::from_millis(10)),
        Duration::from_millis(260)
    );
    assert_eq!(
        backoff.next_sleep_with_jitter(Duration::from_millis(10)),
        Duration::from_millis(510)
    );
    assert_eq!(
        backoff.next_sleep_with_jitter(Duration::from_millis(500)),
        Duration::from_secs(1)
    );
}

#[test]
fn public_backoff_delay_includes_the_base_and_stays_bounded() {
    let mut backoff = Backoff::new(250, 1_000);

    let delay = backoff.next_sleep();

    assert!(delay >= Duration::from_millis(250));
    assert!(delay <= Duration::from_millis(369));
}

#[test]
fn backoff_reset_restores_the_base_delay() {
    let mut backoff = Backoff::new(250, 1_000);
    let _ = backoff.next_sleep_with_jitter(Duration::ZERO);
    let _ = backoff.next_sleep_with_jitter(Duration::ZERO);
    backoff.reset();

    assert_eq!(
        backoff.next_sleep_with_jitter(Duration::ZERO),
        Duration::from_millis(250)
    );
}

#[test]
fn jitter_is_zero_or_strictly_below_its_bound() {
    assert_eq!(jitter_duration(0), Duration::ZERO);
    for bound in [1, 2, 5, 120] {
        assert!(jitter_duration(bound) < Duration::from_millis(bound));
    }
}

#[test]
fn seeded_jitter_uses_the_evolved_state() {
    set_jitter_seed_for_test(7);
    let expected = evolve_jitter_seed(7) % 11;

    assert_eq!(jitter_duration(11), Duration::from_millis(expected));
}

#[test]
fn retry_log_warns_once_until_reset() {
    let mut log = RetryLog::new(Duration::from_mins(1));
    let warnings = Cell::new(0usize);
    let debugs = Cell::new(0usize);

    assert!(log.log_with(
        || warnings.set(warnings.get() + 1),
        || debugs.set(debugs.get() + 1)
    ));
    assert!(!log.log_with(
        || warnings.set(warnings.get() + 1),
        || debugs.set(debugs.get() + 1)
    ));
    log.reset();
    assert!(log.log_with(
        || warnings.set(warnings.get() + 1),
        || debugs.set(debugs.get() + 1)
    ));
    assert_eq!(warnings.get(), 2);
    assert_eq!(debugs.get(), 1);
}

#[test]
fn retry_log_public_wrapper_reports_warning_then_debug_path() {
    let mut log = RetryLog::new(Duration::from_mins(1));

    assert!(log.warn_or_debug(&"offline", "retrying"));
    assert!(!log.warn_or_debug(&"offline", "retrying"));
}

#[test]
fn nanosecond_seed_is_always_the_input_with_its_low_bit_set() {
    assert_eq!(seed_from_nanos(0), 1);
    assert_eq!(seed_from_nanos(2), 3);
    assert_eq!(seed_from_nanos(8), 9);
    assert_eq!(seed_from_nanos(9), 9);
}
