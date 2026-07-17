//! Shared reconnect timing and retry logging for session-bus clients

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use tracing::{debug, warn};

// All UI processes use one policy so a maximum delay has the same meaning everywhere
pub const BACKOFF_BASE_MS: u64 = 250;
pub const BACKOFF_MAX_MS: u64 = 5_000;
pub const BACKOFF_JITTER_MS: u64 = 120;
pub const RETRY_WARN_INTERVAL_SECS: u64 = 30;
static JITTER_STATE: AtomicU64 = AtomicU64::new(0);

/// Exponential reconnect delay with bounded jitter
pub struct Backoff {
    base: Duration,
    current: Duration,
    max: Duration,
}

impl Backoff {
    #[must_use]
    pub const fn new(base_ms: u64, max_ms: u64) -> Self {
        let base = Duration::from_millis(base_ms);
        Self {
            base,
            current: base,
            max: Duration::from_millis(max_ms),
        }
    }

    pub const fn reset(&mut self) {
        self.current = self.base;
    }

    pub fn next_sleep(&mut self) -> Duration {
        self.next_sleep_with_jitter(jitter_duration(BACKOFF_JITTER_MS))
    }

    pub fn next_sleep_with_jitter(&mut self, jitter: Duration) -> Duration {
        let sleep = self.current;
        self.current = (self.current * 2).min(self.max);
        // Clamp after jitter so the configured maximum remains a real ceiling
        (sleep + jitter).min(self.max)
    }
}

/// Rate-limited logger for retry loops that may stay offline for a long time
pub struct RetryLog {
    interval: Duration,
    last_warn: Instant,
}

impl RetryLog {
    #[must_use]
    pub fn new(interval: Duration) -> Self {
        let mut log = Self {
            interval,
            last_warn: Instant::now(),
        };
        log.reset();
        log
    }

    pub fn reset(&mut self) {
        // The first failure after a healthy session should always be visible
        let now = Instant::now();
        self.last_warn = now.checked_sub(self.interval).unwrap_or(now);
    }

    pub fn warn_or_debug<E: std::fmt::Debug>(&mut self, err: &E, message: &str) -> bool {
        self.log_with(|| warn!(?err, "{message}"), || debug!(?err, "{message}"))
    }

    pub fn log_with<F, G>(&mut self, warn_fn: F, debug_fn: G) -> bool
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

#[must_use]
pub fn jitter_duration(max_ms: u64) -> Duration {
    if max_ms == 0 {
        return Duration::ZERO;
    }
    let jitter_ms = next_jitter_seed().wrapping_rem(max_ms);
    Duration::from_millis(jitter_ms)
}

fn next_jitter_seed() -> u64 {
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

const fn seed_from_nanos(nanos: u64) -> u64 {
    nanos | 1
}

const fn evolve_jitter_seed(seed: u64) -> u64 {
    seed.wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407)
}

#[cfg(test)]
#[path = "tests/reconnect.rs"]
mod tests;
