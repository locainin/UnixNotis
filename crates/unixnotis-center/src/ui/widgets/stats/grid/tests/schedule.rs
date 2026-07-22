//! Statistic refresh scheduling tests

use std::time::Duration;

use super::is_due_delay;

#[test]
fn zero_delay_is_due_immediately() {
    assert!(is_due_delay(Some(Duration::ZERO)));
}

#[test]
fn missing_or_positive_delay_is_not_due() {
    assert!(!is_due_delay(None));
    assert!(!is_due_delay(Some(Duration::from_millis(1))));
}
