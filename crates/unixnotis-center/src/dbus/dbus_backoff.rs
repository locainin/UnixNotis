//! Shared backoff and retry logging utilities for D-Bus reconnect logic

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use tracing::{debug, warn};

// Backoff settings throttle reconnect attempts while keeping recovery responsive
pub const BACKOFF_BASE_MS: u64 = 250;
pub const BACKOFF_MAX_MS: u64 = 5000;
pub const BACKOFF_JITTER_MS: u64 = 120;
// Retry warnings are rate-limited to avoid noisy logs during long outages
pub const RETRY_WARN_INTERVAL_SECS: u64 = 30;

/// Exponential backoff with bounded jitter to avoid synchronized reconnects
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
        let sleep = self.current;
        self.current = (self.current * 2).min(self.max);
        sleep + jitter
    }
}

pub fn jitter_duration(max_ms: u64) -> Duration {
    if max_ms == 0 {
        return Duration::from_millis(0);
    }
    // Simple xorshift-based jitter avoids deterministic alignment without extra dependencies
    let jitter_ms = next_jitter_seed().wrapping_rem(max_ms);
    Duration::from_millis(jitter_ms)
}

fn next_jitter_seed() -> u64 {
    static STATE: AtomicU64 = AtomicU64::new(0);
    let fallback = {
        let nanos = u64::from(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .subsec_nanos(),
        );
        // Avoid a zero seed to keep the xorshift cycle moving
        nanos | 1
    };
    advance_jitter_state(&STATE, fallback)
}

fn advance_jitter_state(state: &AtomicU64, fallback: u64) -> u64 {
    let mut current = state.load(Ordering::Relaxed);
    loop {
        let seed = if current == 0 { fallback | 1 } else { current };
        let next = evolve_jitter_seed(seed);
        match state.compare_exchange_weak(current, next, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => return next,
            Err(observed) => current = observed,
        }
    }
}

const fn evolve_jitter_seed(mut value: u64) -> u64 {
    // xorshift64* variant for compact, fast jitter values
    value ^= value >> 12;
    value ^= value << 25;
    value ^= value >> 27;
    value = value.wrapping_mul(0x2545F4914F6CDD1D);
    value
}

/// Rate-limited logger used to avoid warning spam during retry loops
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
        // Allow the next failure after a success to emit a warning immediately
        self.last_warn = instant_before_or_now(Instant::now(), self.interval);
    }

    pub(crate) fn warn_or_debug<E: std::fmt::Debug>(&mut self, err: &E, message: &str) {
        self.log_with(|| warn!(?err, "{message}"), || debug!(?err, "{message}"));
    }

    pub(crate) fn log_with<F, G>(&mut self, warn_fn: F, debug_fn: G)
    where
        F: FnOnce(),
        G: FnOnce(),
    {
        if self.last_warn.elapsed() >= self.interval {
            self.last_warn = Instant::now();
            warn_fn();
        } else {
            debug_fn();
        }
    }
}

fn instant_before_or_now(now: Instant, interval: Duration) -> Instant {
    // A duration can exceed the platform clock history, so subtraction must saturate safely
    now.checked_sub(interval).unwrap_or(now)
}

#[cfg(test)]
#[path = "tests/backoff.rs"]
mod tests;
