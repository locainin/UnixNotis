use std::collections::HashMap;
use std::time::{Duration, Instant};

use super::{
    NotificationQuota, PrincipalBucket, QuotaPrincipal, QuotaState, TokenBucket,
    CLOSE_SENDER_BURST, GLOBAL_BURST, MAX_TRACKED_PRINCIPALS, OVERFLOW_BURST,
    PRINCIPAL_IDLE_TTL_SECONDS, SENDER_BURST,
};

fn principal(index: u32) -> QuotaPrincipal {
    QuotaPrincipal::new(1_000, index, u64::from(index) + 10)
}

fn principal_bucket(now: Instant) -> PrincipalBucket {
    PrincipalBucket {
        bucket: TokenBucket::new(SENDER_BURST, 1.0, now),
        last_seen: now,
    }
}

fn quota_state(now: Instant) -> QuotaState {
    QuotaState {
        global: TokenBucket::new(GLOBAL_BURST, 1.0, now),
        principals: HashMap::new(),
        overflow: TokenBucket::new(OVERFLOW_BURST, 1.0, now),
    }
}

#[test]
fn principal_bucket_rejects_a_burst_and_refills_over_time() {
    let now = Instant::now();
    let quota = NotificationQuota::new_at(now);
    let caller = principal(10);

    for _ in 0..SENDER_BURST as usize {
        assert!(quota.admit_principal(Some(caller), now));
    }
    assert!(!quota.admit_principal(Some(caller), now));
    assert!(quota.admit_principal(Some(caller), now + Duration::from_millis(50)));
}

#[test]
fn global_bucket_limits_requests_before_identity_resolution() {
    let now = Instant::now();
    let quota = NotificationQuota::new_at(now);

    for _ in 0..GLOBAL_BURST as usize {
        assert!(quota.admit_global(now));
    }
    assert!(!quota.admit_global(now));
    assert!(quota.admit_global(now + Duration::from_millis(17)));
}

#[test]
fn close_requests_use_a_separate_higher_principal_budget() {
    let now = Instant::now();
    let notify = NotificationQuota::new_at(now);
    let close = NotificationQuota::new_close_at(now);
    let caller = principal(10);

    for _ in 0..SENDER_BURST as usize {
        assert!(notify.admit_principal(Some(caller), now));
        assert!(close.admit_principal(Some(caller), now));
    }
    assert!(!notify.admit_principal(Some(caller), now));
    for _ in SENDER_BURST as usize..CLOSE_SENDER_BURST as usize {
        assert!(close.admit_principal(Some(caller), now));
    }
    assert!(!close.admit_principal(Some(caller), now));
}

#[test]
fn unknown_and_overflow_principals_share_one_restricted_bucket() {
    let now = Instant::now();
    let quota = NotificationQuota::new_at(now);

    for index in 0..MAX_TRACKED_PRINCIPALS {
        assert!(quota.admit_principal(Some(principal(index as u32)), now));
    }
    for index in 0..OVERFLOW_BURST as usize {
        let admitted = if index % 2 == 0 {
            quota.admit_principal(None, now)
        } else {
            quota.admit_principal(
                Some(principal((MAX_TRACKED_PRINCIPALS + index) as u32)),
                now,
            )
        };
        assert!(admitted);
    }
    assert!(!quota.admit_principal(None, now));
    assert!(!quota.admit_principal(Some(principal(u32::MAX)), now));
    assert_eq!(
        quota.state.lock().expect("quota state").principals.len(),
        MAX_TRACKED_PRINCIPALS
    );
}

#[test]
fn principal_pruning_removes_only_fully_refilled_idle_entries() {
    let now = Instant::now();
    let mut state = quota_state(now);
    let expired_idle = principal(1);
    let throttled = principal(2);
    let recent = principal(3);
    state.principals.insert(expired_idle, principal_bucket(now));
    let mut depleted = principal_bucket(now);
    depleted.bucket.tokens = 0.0;
    depleted.bucket.refill_per_second = 0.0;
    state.principals.insert(throttled, depleted);
    state
        .principals
        .insert(recent, principal_bucket(now + Duration::from_secs(1)));

    state.prune_principal_buckets(now + Duration::from_secs(PRINCIPAL_IDLE_TTL_SECONDS));

    assert!(!state.principals.contains_key(&expired_idle));
    assert!(state.principals.contains_key(&throttled));
    assert!(state.principals.contains_key(&recent));
}

#[test]
fn capacity_never_evicts_a_live_throttled_principal_for_a_new_burst() {
    let now = Instant::now();
    let quota = NotificationQuota::new_at(now);
    let protected = principal(0);

    for _ in 0..SENDER_BURST as usize {
        assert!(quota.admit_principal(Some(protected), now));
    }
    for index in 1..MAX_TRACKED_PRINCIPALS {
        assert!(quota.admit_principal(Some(principal(index as u32)), now));
    }
    assert!(!quota.admit_principal(Some(protected), now));

    for index in MAX_TRACKED_PRINCIPALS..MAX_TRACKED_PRINCIPALS + 20 {
        let _admitted = quota.admit_principal(Some(principal(index as u32)), now);
    }

    assert!(!quota.admit_principal(Some(protected), now));
    assert!(quota
        .state
        .lock()
        .expect("quota state")
        .principals
        .contains_key(&protected));
}

#[test]
fn reconnect_address_churn_does_not_reset_a_process_principal_bucket() {
    let now = Instant::now();
    let quota = NotificationQuota::new_at(now);
    let same_process = QuotaPrincipal::new(1_000, 42, 99);

    let mut admitted = 0usize;
    for _transport_connection in 0..MAX_TRACKED_PRINCIPALS + 64 {
        admitted += usize::from(quota.admit_principal(Some(same_process), now));
    }

    assert_eq!(admitted, SENDER_BURST as usize);
    assert!(!quota.admit_principal(Some(same_process), now));
    assert_eq!(quota.state.lock().expect("quota state").principals.len(), 1);
}
