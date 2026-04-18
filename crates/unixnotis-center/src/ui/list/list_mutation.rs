//! Notification list update and close mutation logic
//!
//! These paths own entry-level changes after the list has already been constructed

use tracing::debug;
use unixnotis_core::{should_archive_closed_notification, CloseReason, NotificationView};

use super::types::NotificationList;

impl NotificationList {
    pub fn add_or_update(&mut self, notification: NotificationView, is_active: bool) {
        let id = notification.id;
        let existing_entry = self.entries.get(&id);
        let old_group = existing_entry.map(|entry| entry.app_key.clone());
        let was_in_active = existing_entry.map(|entry| entry.is_active).unwrap_or(false);
        let was_in_history = existing_entry.is_some() && !was_in_active;
        // Snapshot ordering state before any mutations; used to decide whether a full rebuild
        // is necessary because rebuilds are expensive for large histories
        let was_front = self.active_order.front().copied() == Some(id);
        let needs_new_key = existing_entry
            .map(|entry| entry.view.app_name != notification.app_name)
            .unwrap_or(false);
        let new_key = if needs_new_key {
            Some(self.intern_key(&notification.app_name))
        } else {
            None
        };

        // Track whether this update changes grouping or ordering. If not, update in place
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
            entry.view = std::rc::Rc::new(notification);
            entry.is_active = is_active;
        } else {
            self.insert_entry(notification, is_active);
        }

        let mut ordering_changed = false;
        if is_active {
            // Reorder only when the notification is not already at the front
            if was_in_history || !was_in_active || !was_front {
                self.history_order.retain(|entry| *entry != id);
                self.active_order.retain(|entry| *entry != id);
                self.active_order.push_front(id);
                ordering_changed = true;
            }
        }

        // Fast path: when the group and ordering are unchanged, update the row and header only
        if existing
            && !group_changed
            && old_is_active == Some(is_active)
            && !ordering_changed
            && self.filter_query.is_none()
            && !self.needs_rebuild
        {
            if let Some(entry) = self.entries.get(&id) {
                // Compute stacked state from the cached grouping instead of rebuilding it
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
                // Update the row object in-place to avoid ListStore churn
                entry.item.update(super::list_item::RowData::notification(
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
                            // Refresh the group header count and sample notification
                            header.update(super::list_item::RowData::group_header(
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

        if existing {
            if let Some(entry) = self.entries.get(&id) {
                let current_key = entry.app_key.clone();
                let current_active = entry.is_active;
                if let (Some(old_key), Some(old_active)) = (old_group.as_ref(), old_is_active) {
                    if old_key.as_ref() != current_key.as_ref() {
                        // App-name changes can move the notification between groups
                        // Rebuild only the two affected group indices
                        self.rebuild_group_index_for_key(old_key);
                        self.rebuild_group_index_for_key(&current_key);
                    } else if old_active != current_active {
                        self.index_remove(&current_key, id, old_active);
                        self.index_insert_front(&current_key, id, current_active);
                    } else if ordering_changed {
                        self.index_move_to_front(&current_key, id, current_active);
                    }
                }
            }
        }

        let current_key = self.entries.get(&id).map(|entry| entry.app_key.clone());
        if let Some(key) = current_key {
            self.dirty_groups.insert(key);
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
        let should_archive = self
            .entries
            .get(&id)
            .map(|entry| {
                should_archive_entry(entry.view.as_ref(), reason, self.transient_to_history)
            })
            .unwrap_or(false);

        if !should_archive {
            // Rows that do not belong in history should disappear fully from the list
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
        // Closed notifications are moved to history front to preserve recency ordering
        self.history_order.push_front(id);
        if let Some(key) = group_key {
            self.index_remove(&key, id, true);
            self.index_insert_front(&key, id, false);
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

    pub fn set_filter_query(&mut self, query: &str) -> bool {
        let normalized = self.normalize_filter_query(query);
        if self.filter_query == normalized {
            return false;
        }
        self.filter_query = normalized;
        // Filter changes affect every group; force a full pass to avoid stale ranges
        self.group_ranges.clear();
        self.request_rebuild();
        true
    }
}

fn should_archive_entry(
    notification: &NotificationView,
    reason: CloseReason,
    transient_to_history: bool,
) -> bool {
    // The center should follow the daemon archive rule instead of guessing locally
    should_archive_closed_notification(reason, notification.is_transient, transient_to_history)
}

#[cfg(test)]
mod tests {
    use unixnotis_core::{Action, NotificationImage};

    use super::*;

    fn make_view(is_transient: bool) -> NotificationView {
        NotificationView {
            id: 7,
            app_name: "Test".to_string(),
            summary: "summary".to_string(),
            body: "body".to_string(),
            actions: vec![Action {
                key: "default".to_string(),
                label: "Open".to_string(),
            }],
            urgency: 1,
            is_transient,
            image: NotificationImage::default(),
        }
    }

    #[test]
    fn transient_rows_follow_config_when_closed() {
        assert!(!should_archive_entry(
            &make_view(true),
            CloseReason::Expired,
            false
        ));
        assert!(should_archive_entry(
            &make_view(true),
            CloseReason::Expired,
            true
        ));
    }

    #[test]
    fn user_dismiss_never_archives_locally() {
        assert!(!should_archive_entry(
            &make_view(false),
            CloseReason::DismissedByUser,
            true
        ));
        assert!(!should_archive_entry(
            &make_view(true),
            CloseReason::DismissedByUser,
            true
        ));
    }
}
