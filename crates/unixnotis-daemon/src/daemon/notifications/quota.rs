//! Bounded notification ingress policy

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;

const GLOBAL_BURST: f64 = 120.0;
const GLOBAL_REFILL_PER_SECOND: f64 = 60.0;
const SENDER_BURST: f64 = 40.0;
const SENDER_REFILL_PER_SECOND: f64 = 20.0;
const MAX_TRACKED_SENDERS: usize = 256;
const SENDER_IDLE_TTL_SECONDS: u64 = 60;
const UNKNOWN_SENDER: &str = "<unknown>";

pub(super) struct NotificationQuota {
    state: Mutex<QuotaState>,
}

struct QuotaState {
    global: TokenBucket,
    senders: HashMap<String, SenderBucket>,
}

struct SenderBucket {
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
    pub(super) fn new() -> Self {
        Self::new_at(Instant::now())
    }

    fn new_at(now: Instant) -> Self {
        Self {
            state: Mutex::new(QuotaState {
                global: TokenBucket::new(GLOBAL_BURST, GLOBAL_REFILL_PER_SECOND, now),
                senders: HashMap::new(),
            }),
        }
    }

    pub(super) fn admit(&self, sender: Option<&str>, now: Instant) -> bool {
        let Ok(mut state) = self.state.lock() else {
            // A poisoned limiter fails closed instead of disabling ingress control
            return false;
        };
        state.global.refill(now);
        if !state.global.has_token() {
            return false;
        }

        let sender = sender.unwrap_or(UNKNOWN_SENDER);
        state.prune_sender_buckets(now);
        state.ensure_sender_capacity(sender);
        let sender_bucket =
            state
                .senders
                .entry(sender.to_string())
                .or_insert_with(|| SenderBucket {
                    bucket: TokenBucket::new(SENDER_BURST, SENDER_REFILL_PER_SECOND, now),
                    last_seen: now,
                });
        sender_bucket.last_seen = now;
        sender_bucket.bucket.refill(now);
        if !sender_bucket.bucket.take_token() {
            return false;
        }

        // The global token is consumed only after the sender also passes
        state.global.take_token()
    }
}

impl QuotaState {
    fn prune_sender_buckets(&mut self, now: Instant) {
        self.senders.retain(|_sender, bucket| {
            now.saturating_duration_since(bucket.last_seen).as_secs() < SENDER_IDLE_TTL_SECONDS
        });
    }

    fn ensure_sender_capacity(&mut self, sender: &str) {
        if self.senders.contains_key(sender) || self.senders.len() < MAX_TRACKED_SENDERS {
            return;
        }
        // A bounded linear scan is cheaper than unbounded attacker-controlled state
        if let Some(oldest) = self
            .senders
            .iter()
            .min_by_key(|(_sender, bucket)| bucket.last_seen)
            .map(|(sender, _bucket)| sender.clone())
        {
            self.senders.remove(&oldest);
        }
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
