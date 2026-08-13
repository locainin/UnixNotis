//! Bounded notification ingress policy

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;

const GLOBAL_BURST: f64 = 120.0;
const GLOBAL_REFILL_PER_SECOND: f64 = 60.0;
const SENDER_BURST: f64 = 40.0;
const SENDER_REFILL_PER_SECOND: f64 = 20.0;
const CLOSE_GLOBAL_BURST: f64 = 480.0;
const CLOSE_GLOBAL_REFILL_PER_SECOND: f64 = 240.0;
const CLOSE_ATTEMPT_GLOBAL_BURST: f64 = 960.0;
const CLOSE_ATTEMPT_GLOBAL_REFILL_PER_SECOND: f64 = 480.0;
const CLOSE_SENDER_BURST: f64 = 160.0;
const CLOSE_SENDER_REFILL_PER_SECOND: f64 = 80.0;
const OVERFLOW_BURST: f64 = 10.0;
const OVERFLOW_REFILL_PER_SECOND: f64 = 5.0;
const CLOSE_OVERFLOW_BURST: f64 = 40.0;
const CLOSE_OVERFLOW_REFILL_PER_SECOND: f64 = 20.0;
const MAX_TRACKED_PRINCIPALS: usize = 256;
const PRINCIPAL_IDLE_TTL_SECONDS: u64 = 60;

/// Stable process-lifetime identity used for per-caller ingress fairness
#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq)]
pub(in crate::daemon::notifications) struct QuotaPrincipal {
    uid: u32,
    pid: u32,
    start_time: u64,
}

impl QuotaPrincipal {
    pub(in crate::daemon::notifications) const fn new(uid: u32, pid: u32, start_time: u64) -> Self {
        Self {
            uid,
            pid,
            start_time,
        }
    }
}

pub(in crate::daemon::notifications) struct NotificationQuota {
    state: Mutex<QuotaState>,
    policy: QuotaPolicy,
}

#[derive(Clone, Copy)]
struct QuotaPolicy {
    global_burst: f64,
    global_refill_per_second: f64,
    sender_burst: f64,
    sender_refill_per_second: f64,
    overflow_burst: f64,
    overflow_refill_per_second: f64,
    attempt_global_burst: Option<f64>,
    attempt_global_refill_per_second: Option<f64>,
}

struct QuotaState {
    // Mutation work and rejected-request work use separate process-wide ceilings
    global: TokenBucket,
    attempt_global: Option<TokenBucket>,
    principals: HashMap<QuotaPrincipal, PrincipalBucket>,
    overflow: TokenBucket,
}

/// One result describes the complete hierarchical admission decision
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::daemon::notifications) enum Admission {
    Allowed,
    GlobalLimited,
    PrincipalLimited,
}

impl Admission {
    pub(in crate::daemon::notifications) const fn is_allowed(self) -> bool {
        matches!(self, Self::Allowed)
    }
}

struct PrincipalBucket {
    bucket: TokenBucket,
    last_seen: Instant,
}

struct TokenBucket {
    tokens: f64,
    capacity: f64,
    refill_per_second: f64,
    last_refill: Instant,
}

impl NotificationQuota {
    pub(in crate::daemon::notifications) fn new_notify() -> Self {
        Self::new_at(Instant::now())
    }

    pub(in crate::daemon::notifications) fn new_close() -> Self {
        Self::new_close_at(Instant::now())
    }

    fn new_at(now: Instant) -> Self {
        Self::with_policy(
            now,
            QuotaPolicy {
                global_burst: GLOBAL_BURST,
                global_refill_per_second: GLOBAL_REFILL_PER_SECOND,
                sender_burst: SENDER_BURST,
                sender_refill_per_second: SENDER_REFILL_PER_SECOND,
                overflow_burst: OVERFLOW_BURST,
                overflow_refill_per_second: OVERFLOW_REFILL_PER_SECOND,
                attempt_global_burst: None,
                attempt_global_refill_per_second: None,
            },
        )
    }

    fn new_close_at(now: Instant) -> Self {
        Self::with_policy(
            now,
            QuotaPolicy {
                global_burst: CLOSE_GLOBAL_BURST,
                global_refill_per_second: CLOSE_GLOBAL_REFILL_PER_SECOND,
                sender_burst: CLOSE_SENDER_BURST,
                sender_refill_per_second: CLOSE_SENDER_REFILL_PER_SECOND,
                overflow_burst: CLOSE_OVERFLOW_BURST,
                overflow_refill_per_second: CLOSE_OVERFLOW_REFILL_PER_SECOND,
                attempt_global_burst: Some(CLOSE_ATTEMPT_GLOBAL_BURST),
                attempt_global_refill_per_second: Some(CLOSE_ATTEMPT_GLOBAL_REFILL_PER_SECOND),
            },
        )
    }

    fn with_policy(now: Instant, policy: QuotaPolicy) -> Self {
        Self {
            state: Mutex::new(QuotaState {
                global: TokenBucket::new(policy.global_burst, policy.global_refill_per_second, now),
                attempt_global: policy
                    .attempt_global_burst
                    .zip(policy.attempt_global_refill_per_second)
                    .map(|(burst, refill)| TokenBucket::new(burst, refill, now)),
                principals: HashMap::new(),
                overflow: TokenBucket::new(
                    policy.overflow_burst,
                    policy.overflow_refill_per_second,
                    now,
                ),
            }),
            policy,
        }
    }

    pub(in crate::daemon::notifications) fn try_admit_close_attempt(
        &self,
        principal: Option<QuotaPrincipal>,
        now: Instant,
    ) -> Admission {
        let Ok(mut state) = self.state.lock() else {
            return Admission::GlobalLimited;
        };
        state.prune_principal_buckets(now);
        // Process churn cannot mint work after this shared attempt budget is empty
        if !state.attempt_global_has_token(now) {
            return Admission::GlobalLimited;
        }
        if !state.principal_has_token(principal, now, self.policy) {
            return Admission::PrincipalLimited;
        }
        let principal_taken = state.take_principal_token(principal, now, self.policy);
        let attempt_global_taken = state.take_attempt_global_token(now);
        debug_assert!(
            principal_taken,
            "checked close principal token must remain available"
        );
        debug_assert!(
            attempt_global_taken,
            "checked close attempt token must remain available"
        );
        Admission::Allowed
    }

    pub(in crate::daemon::notifications) fn try_admit_notify(
        &self,
        principal: Option<QuotaPrincipal>,
        now: Instant,
    ) -> Admission {
        self.try_admit_hierarchical(principal, now)
    }

    pub(in crate::daemon::notifications) fn try_admit_close_commit(
        &self,
        now: Instant,
    ) -> Admission {
        let Ok(mut state) = self.state.lock() else {
            return Admission::GlobalLimited;
        };
        // Only an authorized close consumes the protected mutation budget
        state.global.refill(now);
        if !state.global.has_token() {
            return Admission::GlobalLimited;
        }
        let global_taken = state.global.take_token();
        debug_assert!(
            global_taken,
            "checked close commit token must remain available"
        );
        Admission::Allowed
    }

    fn try_admit_hierarchical(&self, principal: Option<QuotaPrincipal>, now: Instant) -> Admission {
        let Ok(mut state) = self.state.lock() else {
            return Admission::GlobalLimited;
        };
        state.prune_principal_buckets(now);
        state.global.refill(now);

        // Check both budgets before decrementing either one
        // A shared rejection also avoids mutating principal LRU admission state
        if !state.global.has_token() {
            return Admission::GlobalLimited;
        }
        if !state.principal_has_token(principal, now, self.policy) {
            return Admission::PrincipalLimited;
        }

        let principal_taken = state.take_principal_token(principal, now, self.policy);
        let global_taken = state.global.take_token();
        debug_assert!(
            principal_taken,
            "checked principal token must remain available"
        );
        debug_assert!(global_taken, "checked global token must remain available");
        Admission::Allowed
    }
}

impl QuotaState {
    fn attempt_global_has_token(&mut self, now: Instant) -> bool {
        self.attempt_global.as_mut().is_some_and(|bucket| {
            bucket.refill(now);
            bucket.has_token()
        })
    }

    fn take_attempt_global_token(&mut self, now: Instant) -> bool {
        self.attempt_global.as_mut().is_some_and(|bucket| {
            bucket.refill(now);
            bucket.take_token()
        })
    }

    fn prune_principal_buckets(&mut self, now: Instant) {
        self.principals.retain(|_principal, bucket| {
            bucket.bucket.refill(now);
            let idle = now.saturating_duration_since(bucket.last_seen).as_secs()
                >= PRINCIPAL_IDLE_TTL_SECONDS;
            // Only a fully restored idle principal may release its bounded map slot
            !(idle && bucket.bucket.is_full())
        });
    }

    fn principal_has_token(
        &mut self,
        principal: Option<QuotaPrincipal>,
        now: Instant,
        policy: QuotaPolicy,
    ) -> bool {
        self.principal_bucket_mut(principal, now, policy)
            .is_some_and(|bucket| bucket.has_token())
    }

    fn take_principal_token(
        &mut self,
        principal: Option<QuotaPrincipal>,
        now: Instant,
        policy: QuotaPolicy,
    ) -> bool {
        self.principal_bucket_mut(principal, now, policy)
            .is_some_and(TokenBucket::take_token)
    }

    fn principal_bucket_mut(
        &mut self,
        principal: Option<QuotaPrincipal>,
        now: Instant,
        policy: QuotaPolicy,
    ) -> Option<&mut TokenBucket> {
        let Some(principal) = principal else {
            // Callers without stable process evidence share one deliberately small allowance
            self.overflow.refill(now);
            return Some(&mut self.overflow);
        };

        if !self.principals.contains_key(&principal)
            && self.principals.len() >= MAX_TRACKED_PRINCIPALS
        {
            // Stable newcomers displace the least-recent entry instead of falling off a quota cliff
            if let Some(oldest) = self
                .principals
                .iter()
                .min_by_key(|(_key, bucket)| bucket.last_seen)
                .map(|(key, _bucket)| *key)
            {
                self.principals.remove(&oldest);
            }
        }

        let principal_bucket =
            self.principals
                .entry(principal)
                .or_insert_with(|| PrincipalBucket {
                    bucket: TokenBucket::new(
                        policy.sender_burst,
                        policy.sender_refill_per_second,
                        now,
                    ),
                    last_seen: now,
                });
        principal_bucket.last_seen = now;
        principal_bucket.bucket.refill(now);
        Some(&mut principal_bucket.bucket)
    }
}

impl TokenBucket {
    const fn new(capacity: f64, refill_per_second: f64, now: Instant) -> Self {
        Self {
            tokens: capacity,
            capacity,
            refill_per_second,
            last_refill: now,
        }
    }

    fn refill(&mut self, now: Instant) {
        let elapsed = now.saturating_duration_since(self.last_refill);
        self.tokens = elapsed
            .as_secs_f64()
            .mul_add(self.refill_per_second, self.tokens)
            .min(self.capacity);
        self.last_refill = now;
    }

    fn has_token(&self) -> bool {
        self.tokens >= 1.0
    }

    fn is_full(&self) -> bool {
        self.tokens >= self.capacity
    }

    fn take_token(&mut self) -> bool {
        if !self.has_token() {
            return false;
        }
        self.tokens -= 1.0;
        true
    }
}

#[cfg(test)]
#[path = "tests/quota.rs"]
mod tests;
