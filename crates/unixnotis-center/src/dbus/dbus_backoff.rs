//! Shared backoff and retry logging utilities for D-Bus reconnect logic.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use tracing::{debug, warn};

// Backoff settings throttle reconnect attempts while keeping recovery responsive.
pub(crate) const BACKOFF_BASE_MS: u64 = 250;
pub(crate) const BACKOFF_MAX_MS: u64 = 5000;
pub(crate) const BACKOFF_JITTER_MS: u64 = 120;
// Retry warnings are rate-limited to avoid noisy logs during long outages.
pub(crate) const RETRY_WARN_INTERVAL_SECS: u64 = 30;

/// Exponential backoff with bounded jitter to avoid synchronized reconnects.
pub(crate) struct Backoff {
    base: Duration,
    current: Duration,
    max: Duration,
}

impl Backoff {
    pub(crate) fn new(base_ms: u64, max_ms: u64) -> Self {
        let base = Duration::from_millis(base_ms);
        Self {
            base,
            current: base,
            max: Duration::from_millis(max_ms),
        }
    }

    pub(crate) fn reset(&mut self) {
        self.current = self.base;
    }

    pub(crate) fn next_sleep(&mut self) -> Duration {
        let jitter = jitter_duration(BACKOFF_JITTER_MS);
        let sleep = self.current;
        self.current = (self.current * 2).min(self.max);
        sleep + jitter
    }
}

pub(crate) fn jitter_duration(max_ms: u64) -> Duration {
    if max_ms == 0 {
        return Duration::from_millis(0);
    }
    // Simple xorshift-based jitter avoids deterministic alignment without extra dependencies.
    let jitter_ms = next_jitter_seed().wrapping_rem(max_ms);
    Duration::from_millis(jitter_ms)
}

fn next_jitter_seed() -> u64 {
    static STATE: AtomicU64 = AtomicU64::new(0);
    // Seed from wall clock once; subsequent calls evolve the state.
    let seed = STATE.load(Ordering::Relaxed);
    let mut value = if seed == 0 {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos() as u64;
        // Avoid a zero seed to keep the xorshift cycle moving.
        nanos | 1
    } else {
        seed
    };
    // xorshift64* variant for compact, fast jitter values.
    value ^= value >> 12;
    value ^= value << 25;
    value ^= value >> 27;
    value = value.wrapping_mul(0x2545F4914F6CDD1D);
    STATE.store(value, Ordering::Relaxed);
    value
}

/// Rate-limited logger used to avoid warning spam during retry loops.
pub(crate) struct RetryLog {
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
        // Allow the next failure after a success to emit a warning immediately.
        self.last_warn = Instant::now() - self.interval;
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

#[cfg(test)]
#[path = "tests/backoff.rs"]
mod tests;
