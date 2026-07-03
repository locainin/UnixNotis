//! Notification list seeding, insertion, and limit trimming
//!
//! These helpers own the base storage lifecycle so mutation code can stay focused on updates

use std::collections::{HashMap, VecDeque};
use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};

use tracing::debug;
use unixnotis_core::NotificationView;

use super::item::{RowData, RowItem, RowPresentation};
use super::types::{NotificationEntry, NotificationList};

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
            // Trim and rebuild when limits change so list size is immediately correct
            self.trim_to_limits();
            self.request_rebuild();
        }
    }

    pub fn seed(&mut self, active: Vec<NotificationView>, history: Vec<NotificationView>) {
        // Reset caches before rebuilding to avoid stale list store content
        self.entries.clear();
        self.active_order.clear();
        self.history_order.clear();
        clear_seed_group_expansion(&mut self.group_expanded);
        self.group_headers.clear();
        self.group_order.clear();
        self.group_order_scratch.clear();
        self.clear_group_indices();
        self.group_ranges.clear();
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

    pub(super) fn trim_to_limits(&mut self) {
        // Active order is newest-first, so trimming from the back drops oldest rows
        for id in drain_order_over_limit(&mut self.active_order, self.max_active) {
            if let Some(entry) = self.entries.remove(&id) {
                self.index_remove(&entry.app_key, id, entry.is_active);
                self.dirty_groups.insert(entry.app_key);
            }
        }

        // History uses the same newest-first order and capacity rule as active rows
        for id in drain_order_over_limit(&mut self.history_order, self.max_entries) {
            if let Some(entry) = self.entries.remove(&id) {
                self.index_remove(&entry.app_key, id, entry.is_active);
                self.dirty_groups.insert(entry.app_key);
            }
        }
    }

    pub(super) fn insert_entry(
        &mut self,
        notification: NotificationView,
        is_active: bool,
    ) -> Rc<str> {
        let id = notification.id;
        let app_key = self.intern_key(&notification.app_name);
        let view = Rc::new(notification);
        let received_at_ms = now_millis();
        let presentation = RowPresentation {
            received_at_ms,
            show_metadata: self.show_notification_metadata,
            show_thumbnail: self.show_notification_thumbnails,
        };
        let item = RowItem::new(RowData::notification(
            app_key.clone(),
            view.clone(),
            false,
            0,
            false,
            is_active,
            presentation,
        ));
        let entry = NotificationEntry {
            view,
            is_active,
            received_at_ms,
            app_key: app_key.clone(),
            item,
        };
        self.entries.insert(id, entry);
        if is_active {
            self.active_order.push_front(id);
        } else {
            self.history_order.push_front(id);
        }
        self.index_insert_front(&app_key, id, is_active);
        app_key
    }

    pub(super) fn remove_entry(&mut self, id: u32) {
        if let Some(entry) = self.entries.remove(&id) {
            self.index_remove(&entry.app_key, id, entry.is_active);
        }
        self.active_order.retain(|entry| *entry != id);
        self.history_order.retain(|entry| *entry != id);
    }
}

fn now_millis() -> i64 {
    // Local receipt time avoids adding timestamp fields to the D-Bus model
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_millis()).ok())
        .unwrap_or(0)
}

fn clear_seed_group_expansion(group_expanded: &mut HashMap<Rc<str>, bool>) {
    group_expanded.clear();
}

fn drain_order_over_limit(order: &mut VecDeque<u32>, max_entries: usize) -> Vec<u32> {
    if max_entries == 0 {
        // A zero limit means the section is disabled, so every id must leave storage
        return order.drain(..).collect();
    }
    if order.len() <= max_entries {
        return Vec::new();
    }

    let remove_count = order.len() - max_entries;
    let mut drained = Vec::with_capacity(remove_count);
    for _ in 0..remove_count {
        // Orders store newest at the front, so the back is always the oldest id
        if let Some(id) = order.pop_back() {
            drained.push(id);
        }
    }
    drained
}

#[cfg(test)]
#[path = "tests/lifecycle.rs"]
mod tests;
