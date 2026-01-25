//! Notification list mutation and bookkeeping.
//!
//! Keeps state updates and list mutations separate from rendering logic.

use std::rc::Rc;

use tracing::debug;
use unixnotis_core::{CloseReason, NotificationView};

use super::list_item::{RowData, RowItem};
use super::{NotificationEntry, NotificationList};

impl NotificationList {
    pub fn apply_limits(&mut self, max_active: usize, max_entries: usize) {
        let mut changed = false;
        if self.max_active != max_active {
            self.max_active = max_active;
            changed = true;
        }
        if self.max_entries != max_entries {
            self.max_entries = max_entries;
            changed = true;
        }
        if changed {
            // Trim and rebuild when limits change so list size is immediately correct.
            self.trim_to_limits();
            self.request_rebuild();
        }
    }

    pub fn seed(&mut self, active: Vec<NotificationView>, history: Vec<NotificationView>) {
        // Reset caches before rebuilding to avoid stale list store content.
        self.entries.clear();
        self.active_order.clear();
        self.history_order.clear();
        self.group_headers.clear();
        self.group_order.clear();
        self.group_order_scratch.clear();
        self.grouped_cache.clear();
        self.group_ranges.clear();
        self.ghost_items.clear();
        self.interned.clear();
        self.current_keys.clear();
        self.keys_scratch.clear();
        self.store.remove_all();
        self.dirty_groups.clear();

        for notification in active {
            self.insert_entry(notification, true);
        }
        for notification in history {
            self.insert_entry(notification, false);
        }
        self.trim_to_limits();

        debug!(
            active = self.active_order.len(),
            history = self.history_order.len(),
            "seeded notification list"
        );
        self.request_rebuild();
    }

    pub fn add_or_update(&mut self, notification: NotificationView, is_active: bool) {
        let id = notification.id;
        let existing_entry = self.entries.get(&id);
        let old_group = existing_entry.map(|entry| entry.app_key.clone());
        let was_in_active = existing_entry.map(|entry| entry.is_active).unwrap_or(false);
        let was_in_history = existing_entry.is_some() && !was_in_active;
        // Snapshot ordering state before any mutations; used to decide whether a full rebuild
        // is necessary (rebuilds are expensive for large histories).
        let was_front = self.active_order.front().copied() == Some(id);
        let needs_new_key = existing_entry
            .map(|entry| entry.view.app_name != notification.app_name)
            .unwrap_or(false);
        let new_key = if needs_new_key {
            Some(self.intern_key(&notification.app_name))
        } else {
            None
        };

        // Track whether this update changes grouping or ordering. If not, update in place.
        let mut existing = false;
        let mut old_is_active = None;
        let mut group_changed = false;
        if let Some(entry) = self.entries.get_mut(&id) {
            existing = true;
            old_is_active = Some(entry.is_active);
            if let Some(key) = new_key {
                entry.app_key = key;
                group_changed = true;
            }
            entry.view = Rc::new(notification);
            entry.is_active = is_active;
        } else {
            self.insert_entry(notification, is_active);
        }

        let mut ordering_changed = false;
        if is_active {
            // Reorder only when the notification is not already at the front.
            if was_in_history || !was_in_active || !was_front {
                self.history_order.retain(|entry| *entry != id);
                self.active_order.retain(|entry| *entry != id);
                self.active_order.push_front(id);
                ordering_changed = true;
            }
        }

        // Fast path: when the group and ordering are unchanged, update the row and header only.
        if existing
            && !group_changed
            && old_is_active == Some(is_active)
            && !ordering_changed
            && !self.needs_rebuild
        {
            if let Some(entry) = self.entries.get(&id) {
                // Compute stacked state from the cached grouping instead of rebuilding it.
                let stacked = self
                    .grouped_cache
                    .get(&entry.app_key)
                    .map(|ids| {
                        !self
                            .group_expanded
                            .get(&entry.app_key)
                            .copied()
                            .unwrap_or(false)
                            && ids.len() > 1
                    })
                    .unwrap_or(false);
                // Update the row object in-place to avoid ListStore churn.
                entry.item.update(RowData::notification(
                    entry.app_key.clone(),
                    entry.view.clone(),
                    stacked,
                    entry.is_active,
                ));
                if let Some(ids) = self.grouped_cache.get(&entry.app_key) {
                    if ids.first().copied() == Some(id) {
                        let expanded = self
                            .group_expanded
                            .get(&entry.app_key)
                            .copied()
                            .unwrap_or(false);
                        if let Some(header) = self.group_headers.get(&entry.app_key) {
                            // Refresh the group header count and sample notification.
                            header.update(RowData::group_header(
                                entry.app_key.clone(),
                                ids.len(),
                                expanded,
                                entry.view.clone(),
                            ));
                        }
                    }
                }
            }
            debug!(id, active = is_active, "notification updated in place");
            return;
        }

        let current_key = self.entries.get(&id).map(|entry| entry.app_key.clone());
        if let Some(key) = current_key.as_ref() {
            self.dirty_groups.insert(key.clone());
        }
        if group_changed {
            if let Some(old_key) = old_group {
                self.dirty_groups.insert(old_key);
            }
        }
        debug!(id, active = is_active, "notification upserted");
        self.trim_to_limits();
        self.request_rebuild();
    }

    pub fn mark_closed(&mut self, id: u32, reason: CloseReason) {
        let group_key = self.entries.get(&id).map(|entry| entry.app_key.clone());
        if matches!(reason, CloseReason::DismissedByUser) {
            self.remove_entry(id);
            if let Some(key) = group_key {
                self.dirty_groups.insert(key);
            }
            debug!(id, ?reason, "notification removed");
            self.request_rebuild();
            return;
        }

        if let Some(entry) = self.entries.get_mut(&id) {
            entry.is_active = false;
        }
        self.active_order.retain(|entry| *entry != id);
        self.history_order.retain(|entry| *entry != id);
        self.history_order.push_front(id);
        if let Some(key) = group_key {
            self.dirty_groups.insert(key);
        }
        debug!(id, ?reason, "notification archived");
        self.trim_to_limits();
        self.request_rebuild();
    }

    pub fn toggle_group(&mut self, key: &str) {
        let key = self.intern_key(key);
        let expanded = self.group_expanded.entry(key.clone()).or_insert(false);
        *expanded = !*expanded;
        self.dirty_groups.insert(key.clone());
        debug!(app = key.as_ref(), expanded = *expanded, "group toggled");
        self.request_rebuild();
    }

    pub fn total_count(&self) -> usize {
        self.active_order.len() + self.history_order.len()
    }

    fn trim_to_limits(&mut self) {
        if self.max_active == 0 {
            for id in self.active_order.drain(..) {
                if let Some(entry) = self.entries.remove(&id) {
                    self.dirty_groups.insert(entry.app_key);
                }
            }
        } else {
            while self.active_order.len() > self.max_active {
                if let Some(id) = self.active_order.pop_back() {
                    if let Some(entry) = self.entries.remove(&id) {
                        self.dirty_groups.insert(entry.app_key);
                    }
                }
            }
        }

        if self.max_entries == 0 {
            for id in self.history_order.drain(..) {
                if let Some(entry) = self.entries.remove(&id) {
                    self.dirty_groups.insert(entry.app_key);
                }
            }
        } else {
            while self.history_order.len() > self.max_entries {
                if let Some(id) = self.history_order.pop_back() {
                    if let Some(entry) = self.entries.remove(&id) {
                        self.dirty_groups.insert(entry.app_key);
                    }
                }
            }
        }
    }

    fn insert_entry(&mut self, notification: NotificationView, is_active: bool) -> Rc<str> {
        let id = notification.id;
        let app_key = self.intern_key(&notification.app_name);
        let view = Rc::new(notification);
        let item = RowItem::new(RowData::notification(
            app_key.clone(),
            view.clone(),
            false,
            is_active,
        ));
        let entry = NotificationEntry {
            view,
            is_active,
            app_key: app_key.clone(),
            item,
        };
        self.entries.insert(id, entry);
        if is_active {
            self.active_order.push_front(id);
        } else {
            self.history_order.push_front(id);
        }
        app_key
    }

    fn remove_entry(&mut self, id: u32) {
        self.entries.remove(&id);
        self.active_order.retain(|entry| *entry != id);
        self.history_order.retain(|entry| *entry != id);
    }
}
