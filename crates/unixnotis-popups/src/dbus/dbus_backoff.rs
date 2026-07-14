//! Retry and jitter helpers for the popup D-Bus runtime

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use tracing::{debug, warn};

// Backoff settings throttle reconnect attempts while keeping recovery responsive
pub const BACKOFF_BASE_MS: u64 = 250;
pub const BACKOFF_MAX_MS: u64 = 5000;
const BACKOFF_JITTER_MS: u64 = 120;
// Retry warnings are rate-limited to avoid noisy logs during long outages
pub const RETRY_WARN_INTERVAL_SECS: u64 = 30;
static JITTER_STATE: AtomicU64 = AtomicU64::new(0);

pub struct Backoff {
    base: Duration,
    current: Duration,
    max: Duration,
}

impl Backoff {
    pub(crate) const fn new(base_ms: u64, max_ms: u64) -> Self {
        let base = Duration::from_millis(base_ms);
        Self {
            base,
            current: base,
            max: Duration::from_millis(max_ms),
        }
    }

    pub(crate) const fn reset(&mut self) {
        self.current = self.base;
    }

    pub(crate) fn next_sleep(&mut self) -> Duration {
        let jitter = jitter_duration(BACKOFF_JITTER_MS);
        self.next_sleep_with_jitter(jitter)
    }

    pub(super) fn next_sleep_with_jitter(&mut self, jitter: Duration) -> Duration {
        let sleep = self.current;
        self.current = (self.current * 2).min(self.max);
        // Clamp after jitter so the public max stays a real ceiling
        (sleep + jitter).min(self.max)
    }
}

// Rate-limited logger avoids warning floods during retry loops
pub struct RetryLog {
    interval: Duration,
    last_warn: Instant,
}

impl RetryLog {
    pub(crate) fn new(interval: Duration) -> Self {
        let mut log = Self {
            interval,
            last_warn: Instant::now(),
        };
        log.reset();
        log
    }

    pub(crate) fn reset(&mut self) {
        // Allow the next failure after a success to warn right away
        let now = Instant::now();
        self.last_warn = now.checked_sub(self.interval).unwrap_or(now);
    }

    pub(crate) fn warn_or_debug<E: std::fmt::Debug>(&mut self, err: &E, message: &str) -> bool {
        self.log_with(|| warn!(?err, "{message}"), || debug!(?err, "{message}"))
    }

    pub(crate) fn log_with<F, G>(&mut self, warn_fn: F, debug_fn: G) -> bool
    where
        F: FnOnce(),
        G: FnOnce(),
    {
        if self.last_warn.elapsed() >= self.interval {
            self.last_warn = Instant::now();
            warn_fn();
            true
        } else {
            debug_fn();
            false
        }
    }
}

pub fn jitter_duration(max_ms: u64) -> Duration {
    if max_ms == 0 {
        return Duration::from_millis(0);
    }
    // A tiny deterministic generator avoids lock contention and extra runtime dependencies
    let jitter_ms = next_jitter_seed().wrapping_rem(max_ms);
    Duration::from_millis(jitter_ms)
}

fn next_jitter_seed() -> u64 {
    // Seed from wall clock once, then evolve the state on each call
    let nanos = u64::from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos(),
    );
    advance_jitter_state(&JITTER_STATE, seed_from_nanos(nanos))
}

fn advance_jitter_state(state: &AtomicU64, fallback: u64) -> u64 {
    let mut current = state.load(Ordering::Relaxed);
    loop {
        let seed = if current == 0 { fallback } else { current };
        let next = evolve_jitter_seed(seed);
        match state.compare_exchange_weak(current, next, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => return next,
            Err(observed) => current = observed,
        }
    }
}

pub(super) const fn seed_from_nanos(nanos: u64) -> u64 {
    // Avoid a zero seed so the generator keeps moving
    nanos | 1
}

pub(super) const fn evolve_jitter_seed(seed: u64) -> u64 {
    // LCG constants from Numerical Recipes; this only needs cheap decorrelation
    seed.wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407)
}

#[cfg(test)]
pub(super) fn set_jitter_seed_for_test(seed: u64) {
    JITTER_STATE.store(seed, Ordering::Relaxed);
}

#[cfg(test)]
#[path = "tests/backoff.rs"]
mod tests;
