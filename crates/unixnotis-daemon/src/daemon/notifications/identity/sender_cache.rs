//! Bounded sender identity cache keyed by unique D-Bus names

use std::collections::HashMap;
use std::sync::Mutex;

use super::sender::SenderMetadata;

const MAX_CACHED_SENDERS: usize = 256;

pub(in crate::daemon) struct SenderMetadataCache {
    state: Mutex<CacheState>,
}

struct CacheState {
    entries: HashMap<String, CacheEntry>,
    sequence: u64,
}

struct CacheEntry {
    metadata: SenderMetadata,
    last_used: u64,
}

impl SenderMetadataCache {
    pub(in crate::daemon) fn new() -> Self {
        Self {
            state: Mutex::new(CacheState {
                entries: HashMap::new(),
                sequence: 0,
            }),
        }
    }

    pub(super) fn get(&self, sender: &str) -> Option<SenderMetadata> {
        // A poisoned cache fails closed and forces fresh sender resolution
        let mut state = self.state.lock().ok()?;
        let sequence = state.next_sequence();
        let entry = state.entries.get_mut(sender)?;
        entry.last_used = sequence;
        Some(entry.metadata.clone())
    }

    pub(super) fn insert(&self, sender: String, metadata: SenderMetadata) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        let sequence = state.next_sequence();
        // The least recently used connection yields before the fixed bound is exceeded
        if !state.entries.contains_key(&sender) && state.entries.len() >= MAX_CACHED_SENDERS {
            state.evict_oldest();
        }
        state.entries.insert(
            sender,
            CacheEntry {
                metadata,
                last_used: sequence,
            },
        );
    }

    pub(in crate::daemon) fn remove(&self, sender: &str) {
        if let Ok(mut state) = self.state.lock() {
            state.entries.remove(sender);
        }
    }
}

impl CacheState {
    const fn next_sequence(&mut self) -> u64 {
        // Wrapping preserves ordering for realistic cache lifetimes without panicking
        self.sequence = self.sequence.wrapping_add(1);
        self.sequence
    }

    fn evict_oldest(&mut self) {
        // A tiny bounded map keeps a linear selection cheaper than another index
        let oldest = self
            .entries
            .iter()
            .min_by_key(|(_sender, entry)| entry.last_used)
            .map(|(sender, _entry)| sender.clone());
        if let Some(sender) = oldest {
            self.entries.remove(&sender);
        }
    }
}

#[cfg(test)]
#[path = "tests/sender_cache.rs"]
mod tests;
