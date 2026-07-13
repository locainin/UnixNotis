use std::cell::Cell;
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::Duration;

use super::{
    evolve_jitter_seed, jitter_duration, seed_from_nanos, set_jitter_seed_for_test, Backoff,
    RetryLog,
};

fn jitter_test_lock() -> MutexGuard<'static, ()> {
    // Jitter state is process-global, so exact-seed tests must run one at a time
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .expect("jitter test lock should not be poisoned")
}

#[test]
fn jitter_zero_returns_zero() {
    let _guard = jitter_test_lock();

    // Zero jitter is used by tests and must never introduce a surprise delay
    assert_eq!(jitter_duration(0), Duration::from_millis(0));
}

#[test]
fn jitter_duration_is_bounded() {
    let _guard = jitter_test_lock();

    // A non-zero bound remains exclusive so the configured max is a true ceiling
    for bound in [1, 2, 5, 120] {
        assert!(jitter_duration(bound) < Duration::from_millis(bound));
    }
}

#[test]
fn jitter_duration_uses_evolved_seed_value() {
    let _guard = jitter_test_lock();
    set_jitter_seed_for_test(7);

    let expected_seed = evolve_jitter_seed(7);
    let jitter = jitter_duration(11);

    assert_eq!(jitter, Duration::from_millis(expected_seed % 11));
    assert_ne!(jitter, Duration::from_millis(0));
}

#[test]
fn jitter_seed_evolution_is_stable_for_known_seed() {
    let seed = 7;

    assert_eq!(evolve_jitter_seed(seed), 9098160460397411210);
}

#[test]
fn seed_from_nanos_forces_nonzero_odd_seed() {
    assert_eq!(seed_from_nanos(0), 1);
    assert_eq!(seed_from_nanos(2), 3);
    assert_eq!(seed_from_nanos(7), 7);
}

#[test]
fn next_sleep_wrapper_applies_jitter_and_advances_backoff() {
    let _guard = jitter_test_lock();
    set_jitter_seed_for_test(7);
    let mut backoff = Backoff::new(250, 1_000);

    let first = backoff.next_sleep();
    let second = backoff.next_sleep();

    assert_eq!(
        first,
        Duration::from_millis(250 + (9098160460397411210 % 120))
    );
    assert!(second >= Duration::from_millis(500));
    assert!(second <= Duration::from_millis(619));
}

#[test]
fn backoff_sleep_doubles_until_max_and_clamps_jitter() {
    let mut backoff = Backoff::new(250, 1_000);

    // Fixed jitter makes the progression exact and catches accidental arithmetic changes
    assert_eq!(
        backoff.next_sleep_with_jitter(Duration::from_millis(10)),
        Duration::from_millis(260)
    );
    assert_eq!(
        backoff.next_sleep_with_jitter(Duration::from_millis(10)),
        Duration::from_millis(510)
    );
    assert_eq!(
        backoff.next_sleep_with_jitter(Duration::from_millis(10)),
        Duration::from_secs(1)
    );
    assert_eq!(
        backoff.next_sleep_with_jitter(Duration::from_millis(500)),
        Duration::from_secs(1)
    );
}

#[test]
fn backoff_reset_returns_next_sleep_to_base_delay() {
    let mut backoff = Backoff::new(250, 1_000);

    let _ = backoff.next_sleep_with_jitter(Duration::from_millis(0));
    let _ = backoff.next_sleep_with_jitter(Duration::from_millis(0));
    backoff.reset();

    assert_eq!(
        backoff.next_sleep_with_jitter(Duration::from_millis(0)),
        Duration::from_millis(250)
    );
}

#[test]
fn retry_log_warns_immediately_then_debugs_until_reset() {
    let mut log = RetryLog::new(Duration::from_mins(1));
    let warnings = Cell::new(0usize);
    let debugs = Cell::new(0usize);

    assert!(log.log_with(
        || warnings.set(warnings.get() + 1),
        || {
            debugs.set(debugs.get() + 1);
        }
    ));
    assert!(!log.log_with(
        || warnings.set(warnings.get() + 1),
        || {
            debugs.set(debugs.get() + 1);
        }
    ));
    assert_eq!(warnings.get(), 1);
    assert_eq!(debugs.get(), 1);

    // A successful reconnect resets the throttle so the next failure is visible
    log.reset();
    assert!(log.log_with(
        || warnings.set(warnings.get() + 1),
        || {
            debugs.set(debugs.get() + 1);
        }
    ));
    assert_eq!(warnings.get(), 2);
    assert_eq!(debugs.get(), 1);
}

#[test]
fn retry_log_warn_or_debug_reports_warning_status() {
    let mut log = RetryLog::new(Duration::from_mins(1));
    let err = "offline";

    assert!(log.warn_or_debug(&err, "dbus retry"));
    assert!(!log.warn_or_debug(&err, "dbus retry"));
}
