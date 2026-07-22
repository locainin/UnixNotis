//! Statistic grid scheduling

use std::time::{Duration, Instant};

use super::StatGrid;

impl StatGrid {
    pub fn next_refresh_in(&self, now: Instant) -> Option<Duration> {
        self.items
            .iter()
            .filter_map(|item| item.next_refresh_in(now))
            .min()
    }

    pub fn is_due(&self, now: Instant) -> bool {
        is_due_delay(self.next_refresh_in(now))
    }
}

pub(in crate::ui::widgets::stats) fn is_due_delay(delay: Option<Duration>) -> bool {
    delay.is_some_and(|value| value.is_zero())
}

#[cfg(test)]
#[path = "tests/schedule.rs"]
mod tests;
