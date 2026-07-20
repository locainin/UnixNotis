//! Statistic grid refresh waves

use std::time::{Duration, Instant};

use super::super::builtin::group::collect_builtin_groups;
use super::StatGrid;

impl StatGrid {
    pub fn refresh(&self, base_interval: Duration, force: bool) {
        let now = Instant::now();
        let builtin_groups = collect_builtin_groups(&self.items, now, force);

        for item in &self.items {
            if item.is_grouped_builtin(now, force) {
                // Grouped built-ins are refreshed once per source below
                continue;
            }
            // Per-card refresh keeps slow sources from blocking the grid
            item.refresh(base_interval, force);
        }

        for group in builtin_groups.into_values() {
            // One sample fans out to every matching card in the grid
            group.refresh(base_interval);
        }
    }
}
