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
