//! Refresh cadence backoff helpers for command-driven widgets

use std::time::{Duration, Instant};

// Async widgets only need a slow health check while a previous read is still running
// The completion path updates cached deadlines, so frequent rechecks only add timer churn
pub(in crate::ui::widgets) const INFLIGHT_REFRESH_RECHECK: Duration = Duration::from_secs(1);

// Backoff keeps stable widgets from re-running commands on every tick
const REFRESH_BACKOFF_MAX_MULT: u64 = 4;
const REFRESH_BACKOFF_STABLE_AFTER: u8 = 2;
const REFRESH_BACKOFF_ERROR_AFTER: u8 = 2;

#[derive(Debug, Default)]
pub(in crate::ui::widgets) struct RefreshBackoff {
    // Absolute deadline for next allowed refresh
    next_due: Option<Instant>,
    // Consecutive refreshes that produced no visible changes
    stable_streak: u8,
    // Consecutive refresh failures
    error_streak: u8,
    // Current multiplier applied to base interval
    backoff_mult: u64,
}

impl RefreshBackoff {
    pub(in crate::ui::widgets) fn should_refresh(&self, now: Instant, force: bool) -> bool {
        if force {
            return true;
        }
        match self.next_due {
            Some(due) => now >= due,
            None => true,
        }
    }

    pub(in crate::ui::widgets) fn note_success(
        &mut self,
        now: Instant,
        base: Duration,
        changed: bool,
    ) {
        self.error_streak = 0;
        if changed {
            // Reset backoff immediately when content changes so UI feels responsive
            self.stable_streak = 0;
            self.backoff_mult = 1;
        } else {
            self.stable_streak = self.stable_streak.saturating_add(1);
            if self.stable_streak >= REFRESH_BACKOFF_STABLE_AFTER {
                self.backoff_mult = (self.backoff_mult.max(1) * 2).min(REFRESH_BACKOFF_MAX_MULT);
            }
        }
        self.next_due = Some(now + scale_duration(base, self.backoff_mult.max(1)));
    }

    pub(in crate::ui::widgets) fn note_error(&mut self, now: Instant, base: Duration) {
        // Retry quickly for first failures, then increase delay under persistent failures
        self.error_streak = self.error_streak.saturating_add(1);
        self.stable_streak = 0;
        if self.error_streak >= REFRESH_BACKOFF_ERROR_AFTER {
            self.backoff_mult = (self.backoff_mult.max(1) * 2).min(REFRESH_BACKOFF_MAX_MULT);
        } else {
            self.backoff_mult = 1;
        }
        self.next_due = Some(now + scale_duration(base, self.backoff_mult.max(1)));
    }

    pub(in crate::ui::widgets) fn next_due_in(&self, now: Instant) -> Option<Duration> {
        self.next_due.map(|due| due.saturating_duration_since(now))
    }
}

fn scale_duration(base: Duration, mult: u64) -> Duration {
    let base_ms = base.as_millis();
    let scaled = base_ms.saturating_mul(mult as u128);
    Duration::from_millis(scaled.min(u64::MAX as u128) as u64)
}

#[cfg(test)]
#[path = "tests/refresh_backoff.rs"]
mod tests;
