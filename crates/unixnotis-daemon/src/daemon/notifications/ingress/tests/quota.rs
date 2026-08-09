use std::collections::HashMap;
use std::time::{Duration, Instant};

use super::{
    Admission, NotificationQuota, PrincipalBucket, QuotaPrincipal, QuotaState, TokenBucket,
    CLOSE_GLOBAL_BURST, CLOSE_SENDER_BURST, GLOBAL_BURST, MAX_TRACKED_PRINCIPALS,
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
        attempt_global: None,
        principals: HashMap::new(),
        overflow: TokenBucket::new(10.0, 1.0, now),
    }
}

#[test]
fn hierarchical_admission_charges_neither_bucket_when_principal_is_limited() {
    let now = Instant::now();
    let quota = NotificationQuota::new_at(now);
    let caller = principal(10);

    for _request in 0..SENDER_BURST as usize {
        assert_eq!(
            quota.try_admit_notify(Some(caller), now),
            Admission::Allowed
        );
    }
    let global_before = quota.state.lock().expect("quota state").global.tokens;

    assert_eq!(
        quota.try_admit_notify(Some(caller), now),
        Admission::PrincipalLimited
    );
    assert_eq!(
        quota
            .state
            .lock()
            .expect("quota state")
            .global
            .tokens
            .to_bits(),
        global_before.to_bits(),
        "principal rejection must not spend a shared token"
    );
}

#[test]
fn hierarchical_admission_charges_neither_bucket_when_global_is_limited() {
    let now = Instant::now();
    let quota = NotificationQuota::new_at(now);
    let caller = principal(10);
    assert!(quota.try_admit_notify(Some(caller), now).is_allowed());
    {
        let mut state = quota.state.lock().expect("quota state");
        state.global.tokens = 0.0;
    }
    let principal_before = quota
        .state
        .lock()
        .expect("quota state")
        .principals
        .get(&caller)
        .expect("principal bucket")
        .bucket
        .tokens;

    assert_eq!(
        quota.try_admit_notify(Some(caller), now),
        Admission::GlobalLimited
    );
    let state = quota.state.lock().expect("quota state");
    assert_eq!(
        state
            .principals
            .get(&caller)
            .expect("principal bucket")
            .bucket
            .tokens
            .to_bits(),
        principal_before.to_bits(),
        "global rejection must not spend a caller token"
    );
}

#[test]
fn global_rejection_does_not_evict_an_established_principal() {
    let now = Instant::now();
    let quota = NotificationQuota::new_at(now);
    {
        let mut state = quota.state.lock().expect("quota state");
        state.global.tokens = 0.0;
        for index in 0..MAX_TRACKED_PRINCIPALS {
            state.principals.insert(
                principal(index as u32),
                PrincipalBucket {
                    bucket: TokenBucket::new(SENDER_BURST, 1.0, now),
                    last_seen: now + Duration::from_nanos(index as u64),
                },
            );
        }
    }
    let newcomer = principal(u32::MAX);

    assert_eq!(
        quota.try_admit_notify(Some(newcomer), now),
        Admission::GlobalLimited
    );
    let state = quota.state.lock().expect("quota state");
    assert_eq!(state.principals.len(), MAX_TRACKED_PRINCIPALS);
    assert!(state.principals.contains_key(&principal(0)));
    assert!(!state.principals.contains_key(&newcomer));
}

#[test]
fn close_commit_charges_only_global_capacity_and_refills_over_time() {
    let now = Instant::now();
    let quota = NotificationQuota::new_close_at(now);

    for _request in 0..CLOSE_GLOBAL_BURST as usize {
        assert!(quota.try_admit_close_commit(now).is_allowed());
    }
    assert_eq!(quota.try_admit_close_commit(now), Admission::GlobalLimited);
    assert!(quota
        .try_admit_close_commit(now + Duration::from_millis(5))
        .is_allowed());
    assert!(quota
        .state
        .lock()
        .expect("quota state")
        .principals
        .is_empty());
}

#[test]
fn successful_close_sequence_charges_one_principal_token_per_operation() {
    let now = Instant::now();
    let quota = NotificationQuota::new_close_at(now);
    let caller = principal(10);

    for _request in 0..CLOSE_SENDER_BURST as usize {
        assert!(
            quota
                .try_admit_close_attempt(Some(caller), now)
                .is_allowed(),
            "every documented caller burst token must admit one successful close"
        );
        assert!(quota.try_admit_close_commit(now).is_allowed());
    }

    assert_eq!(
        quota.try_admit_close_attempt(Some(caller), now),
        Admission::PrincipalLimited
    );
    let state = quota.state.lock().expect("quota state");
    assert_eq!(
        state.global.tokens.to_bits(),
        (CLOSE_GLOBAL_BURST - CLOSE_SENDER_BURST).to_bits(),
        "each successful close must consume one shared commit token"
    );
}

#[test]
fn close_attempts_use_a_separate_higher_principal_budget() {
    let now = Instant::now();
    let notify = NotificationQuota::new_at(now);
    let close = NotificationQuota::new_close_at(now);
    let caller = principal(10);

    for _request in 0..SENDER_BURST as usize {
        assert!(notify.try_admit_notify(Some(caller), now).is_allowed());
        assert!(close
            .try_admit_close_attempt(Some(caller), now)
            .is_allowed());
    }
    assert_eq!(
        notify.try_admit_notify(Some(caller), now),
        Admission::PrincipalLimited
    );
    for _request in SENDER_BURST as usize..CLOSE_SENDER_BURST as usize {
        assert!(close
            .try_admit_close_attempt(Some(caller), now)
            .is_allowed());
    }
    assert_eq!(
        close.try_admit_close_attempt(Some(caller), now),
        Admission::PrincipalLimited
    );
}

#[test]
fn close_attempt_admission_never_charges_shared_mutation_capacity() {
    let now = Instant::now();
    let quota = NotificationQuota::new_close_at(now);
    let global_before = quota.state.lock().expect("quota state").global.tokens;

    assert!(quota
        .try_admit_close_attempt(Some(principal(10)), now)
        .is_allowed());

    assert_eq!(
        quota
            .state
            .lock()
            .expect("quota state")
            .global
            .tokens
            .to_bits(),
        global_before.to_bits(),
        "an ownership-rejected close must leave the shared mutation budget untouched"
    );
}

#[test]
fn principal_rejection_does_not_charge_shared_close_attempt_capacity() {
    let now = Instant::now();
    let quota = NotificationQuota::new_close_at(now);
    let caller = principal(10);

    for _request in 0..CLOSE_SENDER_BURST as usize {
        assert!(quota
            .try_admit_close_attempt(Some(caller), now)
            .is_allowed());
    }
    let before = quota
        .state
        .lock()
        .expect("quota state")
        .attempt_global
        .as_ref()
        .expect("close attempt bucket")
        .tokens;

    assert_eq!(
        quota.try_admit_close_attempt(Some(caller), now),
        Admission::PrincipalLimited
    );
    assert_eq!(
        quota
            .state
            .lock()
            .expect("quota state")
            .attempt_global
            .as_ref()
            .expect("close attempt bucket")
            .tokens
            .to_bits(),
        before.to_bits(),
        "caller rejection must not spend shared close-attempt capacity"
    );
}

#[test]
fn stable_principal_churn_cannot_mint_unbounded_close_attempt_capacity() {
    let now = Instant::now();
    let quota = NotificationQuota::new_close_at(now);

    for index in 0..960_u32 {
        assert_eq!(
            quota.try_admit_close_attempt(Some(principal(index)), now),
            Admission::Allowed,
            "every documented global attempt token should admit one cold principal"
        );
    }

    let state = quota.state.lock().expect("quota state");
    assert_eq!(state.principals.len(), MAX_TRACKED_PRINCIPALS);
    assert_eq!(state.global.tokens.to_bits(), CLOSE_GLOBAL_BURST.to_bits());
    drop(state);
    assert_eq!(
        quota.try_admit_close_attempt(Some(principal(u32::MAX)), now),
        Admission::GlobalLimited,
        "a new process identity must not create capacity after the attempt budget is empty"
    );
}

#[test]
fn stable_newcomer_displaces_the_least_recent_principal_at_capacity() {
    let now = Instant::now();
    let quota = NotificationQuota::new_close_at(now);
    for index in 0..MAX_TRACKED_PRINCIPALS {
        let observed = now + Duration::from_nanos(index as u64);
        assert!(quota
            .try_admit_close_attempt(Some(principal(index as u32)), observed)
            .is_allowed());
    }
    let newcomer = principal(u32::MAX);

    assert!(quota
        .try_admit_close_attempt(Some(newcomer), now + Duration::from_secs(1))
        .is_allowed());
    let state = quota.state.lock().expect("quota state");
    assert_eq!(state.principals.len(), MAX_TRACKED_PRINCIPALS);
    assert!(!state.principals.contains_key(&principal(0)));
    assert!(state.principals.contains_key(&newcomer));
}

#[test]
fn unknown_principals_remain_in_one_restricted_bucket() {
    let now = Instant::now();
    let quota = NotificationQuota::new_at(now);

    for _request in 0..10 {
        assert!(quota.try_admit_notify(None, now).is_allowed());
    }
    assert_eq!(
        quota.try_admit_notify(None, now),
        Admission::PrincipalLimited
    );
    assert!(quota
        .state
        .lock()
        .expect("quota state")
        .principals
        .is_empty());
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
fn reconnect_address_churn_does_not_reset_a_process_principal_bucket() {
    let now = Instant::now();
    let quota = NotificationQuota::new_at(now);
    let same_process = QuotaPrincipal::new(1_000, 42, 99);

    let mut admitted = 0usize;
    for _transport_connection in 0..MAX_TRACKED_PRINCIPALS + 64 {
        admitted += usize::from(quota.try_admit_notify(Some(same_process), now).is_allowed());
    }

    assert_eq!(admitted, SENDER_BURST as usize);
    assert_eq!(
        quota.try_admit_notify(Some(same_process), now),
        Admission::PrincipalLimited
    );
    assert_eq!(quota.state.lock().expect("quota state").principals.len(), 1);
}
