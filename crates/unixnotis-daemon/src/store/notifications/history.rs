//! Notification history storage with ordering
//!
//! Kept in a dedicated module so store.rs can focus on active notifications
//! and cross-cutting policy decisions

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Weak};

use unixnotis_core::{Notification, NotificationView};

struct HistoryEntry {
    notification: Arc<Notification>,
    // Weak source identity supports race-safe cleanup without retaining live payloads
    source: Weak<Notification>,
}

pub(in crate::store) struct HistoryStore {
    entries: HashMap<u32, HistoryEntry>,
    order: VecDeque<u32>,
}

impl HistoryStore {
    pub(in crate::store) fn new() -> Self {
        Self {
            entries: HashMap::new(),
            order: VecDeque::new(),
        }
    }

    pub(in crate::store) fn len(&self) -> usize {
        self.entries.len()
    }

    pub(in crate::store) fn contains(&self, id: &u32) -> bool {
        self.entries.contains_key(id)
    }

    pub(in crate::store) fn get(&self, id: &u32) -> Option<&Arc<Notification>> {
        self.entries.get(id).map(|entry| &entry.notification)
    }

    pub(in crate::store) fn clear(&mut self) {
        self.entries.clear();
        self.order.clear();
    }

    pub(in crate::store) fn list_views(&self) -> Vec<NotificationView> {
        let mut views = Vec::with_capacity(self.entries.len());
        for id in self.order.iter().rev() {
            if let Some(entry) = self.entries.get(id) {
                views.push(entry.notification.to_list_view());
            }
        }
        views
    }

    pub(in crate::store) fn remove(&mut self, id: &u32) -> Option<Arc<Notification>> {
        let removed = self.entries.remove(id).map(|entry| entry.notification);
        if removed.is_some() {
            // Removal is infrequent compared to insertion; pay the cost here to keep order clean
            self.order.retain(|entry| entry != id);
        }
        removed
    }

    pub(in crate::store) fn insert(&mut self, notification: Arc<Notification>) {
        let id = notification.id;
        if self.entries.contains_key(&id) {
            // Avoid duplicate IDs in order when a notification is replaced
            self.order.retain(|entry| *entry != id);
        }
        self.entries.insert(
            id,
            HistoryEntry {
                notification,
                source: Weak::new(),
            },
        );
        self.order.push_back(id);
    }

    pub(in crate::store) fn set_source(&mut self, id: u32, source: Weak<Notification>) {
        if let Some(entry) = self.entries.get_mut(&id) {
            entry.source = source;
        }
    }

    pub(in crate::store) fn remove_if_source(
        &mut self,
        id: u32,
        expected: &Arc<Notification>,
    ) -> Option<Arc<Notification>> {
        let source_matches = self
            .entries
            .get(&id)
            .and_then(|entry| entry.source.upgrade())
            .is_some_and(|source| Arc::ptr_eq(&source, expected));
        if !source_matches {
            return None;
        }
        self.remove(&id)
    }

    pub(in crate::store) fn evict_to_limit(&mut self, max_entries: usize) {
        if max_entries == 0 {
            self.clear();
            return;
        }

        while self.entries.len() > max_entries {
            let Some(id) = self.order.pop_front() else {
                // Recover ordering when entries outlive the recorded order
                self.order.extend(self.entries.keys().copied());
                if self.order.is_empty() {
                    break;
                }
                continue;
            };

            if self.entries.remove(&id).is_none() {
                continue;
            }
        }

        if self.entries.is_empty() {
            self.order.clear();
        }
    }
}
