use std::time::{Duration, Instant};

use super::{NotificationQuota, GLOBAL_BURST, MAX_TRACKED_SENDERS, SENDER_BURST};

#[test]
fn sender_bucket_rejects_a_burst_and_refills_over_time() {
    let now = Instant::now();
    let quota = NotificationQuota::new_at(now);

    for _ in 0..SENDER_BURST as usize {
        assert!(quota.admit(Some(":1.10"), now));
    }
    assert!(!quota.admit(Some(":1.10"), now));
    assert!(quota.admit(Some(":1.10"), now + Duration::from_millis(50)));
}

#[test]
fn global_bucket_limits_many_independent_senders() {
    let now = Instant::now();
    let quota = NotificationQuota::new_at(now);

    for index in 0..GLOBAL_BURST as usize {
        assert!(quota.admit(Some(&format!(":1.{index}")), now));
    }
    assert!(!quota.admit(Some(":1.blocked"), now));
    assert!(quota.admit(Some(":1.allowed"), now + Duration::from_millis(17)));
}

#[test]
fn sender_tracking_stays_bounded_and_unknown_callers_share_one_bucket() {
    let now = Instant::now();
    let quota = NotificationQuota::new_at(now);

    for index in 0..MAX_TRACKED_SENDERS + 20 {
        let at = now + Duration::from_secs(index as u64);
        assert!(quota.admit(Some(&format!(":1.{index}")), at));
    }
    assert!(quota.state.lock().expect("quota state").senders.len() <= MAX_TRACKED_SENDERS);

    let later = now + Duration::from_secs((MAX_TRACKED_SENDERS + 21) as u64);
    for _ in 0..SENDER_BURST as usize {
        assert!(quota.admit(None, later));
    }
    assert!(!quota.admit(None, later));
}
