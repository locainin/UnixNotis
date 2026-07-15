//! Short-lived cache for missing icon names

use std::collections::{HashSet, VecDeque};
use std::time::{Duration, Instant};

use super::cache::IconKey;

const MISSING_ICON_TTL: Duration = Duration::from_secs(30);

pub(super) struct MissingIconCache {
    order: VecDeque<(IconKey, Instant)>,
    set: HashSet<IconKey>,
    max_entries: usize,
}

impl MissingIconCache {
    pub(super) fn new(max_entries: usize) -> Self {
        Self {
            order: VecDeque::new(),
            set: HashSet::new(),
            max_entries,
        }
    }

    pub(super) fn contains(&mut self, key: &IconKey) -> bool {
        self.purge_expired(Instant::now());
        self.set.contains(key)
    }

    pub(super) fn insert(&mut self, key: IconKey) {
        self.purge_expired(Instant::now());
        if !self.set.insert(key.clone()) {
            return;
        }
        self.order.push_back((key, Instant::now()));
        while self.order.len() > self.max_entries {
            if let Some((evicted, _)) = self.order.pop_front() {
                // Ordered and membership storage must evict together
                self.set.remove(&evicted);
            }
        }
    }

    fn purge_expired(&mut self, now: Instant) {
        while let Some((_, timestamp)) = self.order.front() {
            if now.saturating_duration_since(*timestamp) < MISSING_ICON_TTL {
                break;
            }
            let Some((key, _)) = self.order.pop_front() else {
                break;
            };
            self.set.remove(&key);
        }
    }

    pub(super) fn clear(&mut self) {
        self.order.clear();
        self.set.clear();
    }
}

#[cfg(test)]
#[path = "tests/missing.rs"]
mod tests;
