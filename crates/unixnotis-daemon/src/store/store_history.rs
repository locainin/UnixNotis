//! Notification history storage with ordering.
//!
//! Kept in a dedicated module so store.rs can focus on active notifications
//! and cross-cutting policy decisions.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use unixnotis_core::{Notification, NotificationView};

pub(super) struct HistoryStore {
    entries: HashMap<u32, Arc<Notification>>,
    order: VecDeque<u32>,
}

impl HistoryStore {
    pub(super) fn new() -> Self {
        Self {
            entries: HashMap::new(),
            order: VecDeque::new(),
        }
    }

    pub(super) fn len(&self) -> usize {
        self.entries.len()
    }

    pub(super) fn contains(&self, id: &u32) -> bool {
        self.entries.contains_key(id)
    }

    pub(super) fn clear(&mut self) {
        self.entries.clear();
        self.order.clear();
    }

    pub(super) fn list_views(&self) -> Vec<NotificationView> {
        let mut views = Vec::with_capacity(self.entries.len());
        for id in self.order.iter().rev() {
            if let Some(notification) = self.entries.get(id) {
                views.push(notification.to_list_view());
            }
        }
        views
    }

    pub(super) fn remove(&mut self, id: &u32) -> Option<Arc<Notification>> {
        let removed = self.entries.remove(id);
        if removed.is_some() {
            // Removal is infrequent compared to insertion; pay the cost here to keep order clean.
            self.order.retain(|entry| entry != id);
        }
        removed
    }

    pub(super) fn insert(&mut self, notification: Arc<Notification>) {
        let id = notification.id;
        if self.entries.contains_key(&id) {
            // Avoid duplicate IDs in order when a notification is replaced.
            self.order.retain(|entry| *entry != id);
        }
        self.entries.insert(id, notification);
        self.order.push_back(id);
    }

    pub(super) fn evict_to_limit(&mut self, max_entries: usize) {
        if max_entries == 0 {
            self.clear();
            return;
        }

        while self.entries.len() > max_entries {
            let Some(id) = self.order.pop_front() else {
                // Recover ordering when entries outlive the recorded order.
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
