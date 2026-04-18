//! Incremental per-group index maintenance for the notification list.
//!
//! Keeps grouped ordering caches synchronized with per-notification mutations.

use std::collections::{HashMap, HashSet, VecDeque};
use std::rc::Rc;

use super::types::NotificationList;

impl NotificationList {
    pub(super) fn clear_group_indices(&mut self) {
        // Clear all parallel caches together to keep index invariants aligned.
        self.group_active_index.clear();
        self.group_history_index.clear();
        self.grouped_cache.clear();
    }

    pub(super) fn index_insert_front(&mut self, key: &Rc<str>, id: u32, is_active: bool) {
        let map = if is_active {
            &mut self.group_active_index
        } else {
            &mut self.group_history_index
        };
        let bucket = map.entry(key.clone()).or_insert_with(VecDeque::new);
        if bucket.front().copied() != Some(id) {
            // Deduplicate before front insert so IDs remain unique inside each bucket.
            bucket.retain(|entry| *entry != id);
            bucket.push_front(id);
        }
        self.sync_group_cache_for_key(key);
    }

    pub(super) fn index_remove(&mut self, key: &Rc<str>, id: u32, was_active: bool) {
        let map = if was_active {
            &mut self.group_active_index
        } else {
            &mut self.group_history_index
        };
        remove_from_group_bucket(map, key, id);
        self.sync_group_cache_for_key(key);
    }

    pub(super) fn index_move_to_front(&mut self, key: &Rc<str>, id: u32, is_active: bool) {
        let map = if is_active {
            &mut self.group_active_index
        } else {
            &mut self.group_history_index
        };
        let bucket = map.entry(key.clone()).or_insert_with(VecDeque::new);
        if bucket.front().copied() == Some(id) {
            return;
        }
        bucket.retain(|entry| *entry != id);
        bucket.push_front(id);
        self.sync_group_cache_for_key(key);
    }

    pub(super) fn rebuild_group_index_for_key(&mut self, key: &Rc<str>) {
        let mut active_ids = VecDeque::new();
        let mut history_ids = VecDeque::new();
        for id in &self.active_order {
            if let Some(entry) = self.entries.get(id) {
                if entry.app_key.as_ref() == key.as_ref() {
                    active_ids.push_back(*id);
                }
            }
        }
        for id in &self.history_order {
            if let Some(entry) = self.entries.get(id) {
                if entry.app_key.as_ref() == key.as_ref() {
                    history_ids.push_back(*id);
                }
            }
        }
        if active_ids.is_empty() {
            self.group_active_index.remove(key);
        } else {
            self.group_active_index.insert(key.clone(), active_ids);
        }
        if history_ids.is_empty() {
            self.group_history_index.remove(key);
        } else {
            self.group_history_index.insert(key.clone(), history_ids);
        }
        self.sync_group_cache_for_key(key);
    }

    pub(super) fn sync_group_cache_for_key(&mut self, key: &Rc<str>) {
        let active = self.group_active_index.get(key);
        let history = self.group_history_index.get(key);
        let total =
            active.map(|ids| ids.len()).unwrap_or(0) + history.map(|ids| ids.len()).unwrap_or(0);
        if total == 0 {
            self.grouped_cache.remove(key);
            return;
        }
        let mut merged = self.grouped_cache.remove(key).unwrap_or_default();
        merged.clear();
        merged.reserve(total);
        if let Some(ids) = active {
            // Active rows are always listed before history rows inside one group.
            merged.extend(ids.iter().copied());
        }
        if let Some(ids) = history {
            merged.extend(ids.iter().copied());
        }
        self.grouped_cache.insert(key.clone(), merged);
    }

    pub(super) fn collect_group_order(&self, out: &mut Vec<Rc<str>>) {
        out.clear();
        let mut seen = HashSet::<Rc<str>>::new();
        for id in self.active_order.iter().chain(self.history_order.iter()) {
            let Some(entry) = self.entries.get(id) else {
                continue;
            };
            let key = entry.app_key.clone();
            let Some(ids) = self.grouped_cache.get(&key) else {
                continue;
            };
            if ids.is_empty() {
                continue;
            }
            if !self.group_has_visible_entries(ids) {
                continue;
            }
            if seen.insert(key.clone()) {
                // First-seen ordering preserves recency across both active/history queues.
                out.push(key);
            }
        }
    }
}

fn remove_from_group_bucket(map: &mut HashMap<Rc<str>, VecDeque<u32>>, key: &Rc<str>, id: u32) {
    if let Some(bucket) = map.get_mut(key) {
        bucket.retain(|entry| *entry != id);
        if bucket.is_empty() {
            map.remove(key);
        }
    }
}
