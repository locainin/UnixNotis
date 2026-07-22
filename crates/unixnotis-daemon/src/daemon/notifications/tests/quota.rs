use std::time::{Duration, Instant};

use std::collections::HashMap;

use super::{
    NotificationQuota, QuotaState, SenderBucket, TokenBucket, GLOBAL_BURST, MAX_TRACKED_SENDERS,
    SENDER_BURST, SENDER_IDLE_TTL_SECONDS,
};

fn sender_bucket(now: Instant) -> SenderBucket {
    SenderBucket {
        bucket: TokenBucket::new(SENDER_BURST, 1.0, now),
        last_seen: now,
    }
}

fn quota_state(now: Instant) -> QuotaState {
    QuotaState {
        global: TokenBucket::new(GLOBAL_BURST, 1.0, now),
        senders: HashMap::new(),
    }
}

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

#[test]
fn sender_pruning_removes_entries_at_the_idle_boundary_only() {
    let now = Instant::now();
    let mut state = quota_state(now);
    state
        .senders
        .insert("stale".to_string(), sender_bucket(now));
    state.senders.insert(
        "recent".to_string(),
        sender_bucket(now + Duration::from_secs(1)),
    );

    state.prune_sender_buckets(now + Duration::from_secs(SENDER_IDLE_TTL_SECONDS));

    assert!(!state.senders.contains_key("stale"));
    assert!(state.senders.contains_key("recent"));
}

#[test]
fn sender_capacity_preserves_existing_and_below_limit_sets() {
    let now = Instant::now();
    let mut state = quota_state(now);
    state
        .senders
        .insert("existing".to_string(), sender_bucket(now));

    state.ensure_sender_capacity("new");
    assert_eq!(state.senders.len(), 1);
    assert!(state.senders.contains_key("existing"));

    for index in 1..MAX_TRACKED_SENDERS {
        state.senders.insert(
            format!("sender-{index}"),
            sender_bucket(now + Duration::from_secs(index as u64)),
        );
    }
    let before = state.senders.len();
    state.ensure_sender_capacity("existing");

    assert_eq!(state.senders.len(), before);
    assert!(state.senders.contains_key("existing"));
}

#[test]
fn sender_capacity_evicts_the_oldest_entry_at_the_exact_limit() {
    let now = Instant::now();
    let mut state = quota_state(now);
    for index in 0..MAX_TRACKED_SENDERS {
        state.senders.insert(
            format!("sender-{index}"),
            sender_bucket(now + Duration::from_secs(index as u64)),
        );
    }

    state.ensure_sender_capacity("new");

    assert_eq!(state.senders.len(), MAX_TRACKED_SENDERS - 1);
    assert!(!state.senders.contains_key("sender-0"));
    assert!(state.senders.contains_key("sender-1"));
}
