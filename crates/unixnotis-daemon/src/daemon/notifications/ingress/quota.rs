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
}

struct QuotaState {
    global: TokenBucket,
    principals: HashMap<QuotaPrincipal, PrincipalBucket>,
    overflow: TokenBucket,
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
            },
        )
    }

    fn with_policy(now: Instant, policy: QuotaPolicy) -> Self {
        Self {
            state: Mutex::new(QuotaState {
                global: TokenBucket::new(policy.global_burst, policy.global_refill_per_second, now),
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

    pub(in crate::daemon::notifications) fn admit_global(&self, now: Instant) -> bool {
        let Ok(mut state) = self.state.lock() else {
            // A poisoned limiter fails closed instead of disabling ingress control
            return false;
        };
        state.global.refill(now);
        state.global.take_token()
    }

    pub(in crate::daemon::notifications) fn admit_principal(
        &self,
        principal: Option<QuotaPrincipal>,
        now: Instant,
    ) -> bool {
        let Ok(mut state) = self.state.lock() else {
            return false;
        };
        state.prune_principal_buckets(now);
        let Some(principal) = principal else {
            state.overflow.refill(now);
            return state.overflow.take_token();
        };
        if !state.principals.contains_key(&principal)
            && state.principals.len() >= MAX_TRACKED_PRINCIPALS
        {
            // D-Bus unique names are ephemeral transport addresses, not stable principals
            // Unknown or overflow identities share a restricted bucket rather than receiving
            // a fresh burst
            state.overflow.refill(now);
            return state.overflow.take_token();
        }
        let principal_bucket =
            state
                .principals
                .entry(principal)
                .or_insert_with(|| PrincipalBucket {
                    bucket: TokenBucket::new(
                        self.policy.sender_burst,
                        self.policy.sender_refill_per_second,
                        now,
                    ),
                    last_seen: now,
                });
        principal_bucket.last_seen = now;
        principal_bucket.bucket.refill(now);
        principal_bucket.bucket.take_token()
    }
}

impl QuotaState {
    fn prune_principal_buckets(&mut self, now: Instant) {
        self.principals.retain(|_principal, bucket| {
            bucket.bucket.refill(now);
            let idle = now.saturating_duration_since(bucket.last_seen).as_secs()
                >= PRINCIPAL_IDLE_TTL_SECONDS;
            // Only a fully restored idle principal may release its bounded map slot
            !(idle && bucket.bucket.is_full())
        });
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
