//! Notification list rebuild and incremental update logic.
//!
//! Keeps list-store mutation logic separate from data mutation methods.

use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use gio::prelude::ListModelExt;
use gtk::glib;
use gtk::glib::object::Cast;
use gtk::prelude::WidgetExt;
use tracing::debug;

use super::list_blocks;
use super::types::{GroupRange, NotificationList, RowKey};
use super::RowItem;

impl NotificationList {
    pub fn flush_rebuild(&mut self) {
        if !self.needs_rebuild {
            return;
        }
        self.needs_rebuild = false;
        if self.store.n_items() == 0 || self.group_ranges.is_empty() {
            self.rebuild_list();
            return;
        }
        self.apply_updates();
    }

    pub fn needs_rebuild(&self) -> bool {
        self.needs_rebuild
    }

    fn rebuild_list(&mut self) {
        let mut group_order = std::mem::take(&mut self.group_order_scratch);
        self.collect_group_order(&mut group_order);

        self.group_headers
            .retain(|key, _| self.grouped_cache.contains_key(key));
        self.group_expanded
            .retain(|key, _| self.grouped_cache.contains_key(key));

        let mut items = std::mem::take(&mut self.items_scratch);
        items.clear();
        let mut keys = std::mem::take(&mut self.keys_scratch);
        keys.clear();
        let mut group_ranges = HashMap::new();
        for key in &group_order {
            let Some(ids) = self.grouped_cache.get(key) else {
                continue;
            };
            let visible_ids = self.visible_ids_for_group(ids).into_owned();
            if visible_ids.is_empty() {
                continue;
            }
            // Range map allows incremental updates to splice only changed groups later.
            let start = items.len();
            let (block_items, block_keys) = self.build_group_block(key, &visible_ids);
            items.extend(block_items);
            keys.extend(block_keys);
            let end = items.len();
            group_ranges.insert(
                key.clone(),
                GroupRange {
                    start,
                    len: end - start,
                },
            );
        }

        let mut current_keys = std::mem::take(&mut self.current_keys);
        let (prefix, suffix) = list_blocks::common_prefix_suffix(&current_keys, &keys);
        let current_mid = current_keys.len().saturating_sub(prefix + suffix);
        let next_mid = keys.len().saturating_sub(prefix + suffix);
        if current_mid != 0 || next_mid != 0 {
            let mut objects = std::mem::take(&mut self.objects_scratch);
            objects.clear();
            // Splice only the changed middle segment to reduce GTK churn.
            for item in &items[prefix..prefix + next_mid] {
                objects.push(item.clone().upcast::<glib::Object>());
            }
            let position = prefix as u32;
            let removals = current_mid as u32;
            self.store.splice(position, removals, &objects);
            objects.clear();
            self.objects_scratch = objects;
        } else {
            self.objects_scratch.clear();
        }

        current_keys.clear();
        self.current_keys = keys;
        self.keys_scratch = current_keys;
        items.clear();
        self.items_scratch = items;

        let group_count = group_order.len();
        let mut old_group_order = std::mem::replace(&mut self.group_order, group_order);
        // Drop stale group keys while keeping the scratch capacity for reuse.
        old_group_order.clear();
        self.group_order_scratch = old_group_order;
        self.group_ranges = group_ranges;
        self.ghost_items
            .retain(|(key, _), _| self.grouped_cache.contains_key(key));

        // Prune interned keys that are no longer referenced by any list state.
        self.interned.retain(|key| Rc::strong_count(key) > 1);
        self.dirty_groups.clear();

        self.update_empty_overlay();

        debug!(
            groups = group_count,
            active = self.active_order.len(),
            history = self.history_order.len(),
            "rebuilt notification list"
        );
    }

    fn apply_updates(&mut self) {
        // Rebuild only affected group blocks while keeping stable spans intact.
        let mut group_order = std::mem::take(&mut self.group_order_scratch);
        self.collect_group_order(&mut group_order);

        self.group_headers
            .retain(|key, _| self.grouped_cache.contains_key(key));
        self.group_expanded
            .retain(|key, _| self.grouped_cache.contains_key(key));

        let mut keep_groups: HashSet<Rc<str>> = HashSet::new();
        let mut removed_groups: HashSet<Rc<str>> = HashSet::new();
        let mut remove_ranges: Vec<GroupRange> = Vec::new();
        for (key, range) in self.group_ranges.iter() {
            let Some(ids) = self.grouped_cache.get(key) else {
                remove_ranges.push(*range);
                removed_groups.insert(key.clone());
                continue;
            };
            let visible_ids = self.visible_ids_for_group(ids);
            if visible_ids.is_empty() {
                remove_ranges.push(*range);
                removed_groups.insert(key.clone());
                continue;
            }
            let desired_len = self.group_block_len(key, visible_ids.as_ref());
            if !self.dirty_groups.contains(key) && range.len == desired_len {
                // Stable groups with identical span lengths are kept in place.
                keep_groups.insert(key.clone());
            } else {
                // Dirty or shape-changed groups are removed and rebuilt.
                remove_ranges.push(*range);
                removed_groups.insert(key.clone());
            }
        }

        remove_ranges.sort_by_key(|range| range.start);
        let mut merged: Vec<GroupRange> = Vec::new();
        for range in remove_ranges {
            if let Some(last) = merged.last_mut() {
                if last.start + last.len == range.start {
                    // Adjacent removals are merged to reduce ListStore splice calls.
                    last.len += range.len;
                    continue;
                }
            }
            merged.push(range);
        }
        for range in merged.into_iter().rev() {
            self.remove_block(range.start, range.len);
        }
        for key in removed_groups {
            self.group_ranges.remove(&key);
        }

        let mut cursor = 0usize;
        let mut new_ranges = HashMap::with_capacity(group_order.len());
        let mut pending_items: Vec<RowItem> = Vec::new();
        let mut pending_keys: Vec<RowKey> = Vec::new();
        let mut pending_start = 0usize;

        for key in &group_order {
            let Some(ids) = self.grouped_cache.get(key) else {
                continue;
            };
            let visible_ids = self.visible_ids_for_group(ids).into_owned();
            if visible_ids.is_empty() {
                continue;
            }
            let desired_len = self.group_block_len(key, &visible_ids);
            if keep_groups.contains(key) {
                if !pending_items.is_empty() {
                    let inserted_len =
                        self.insert_block(pending_start, &pending_items, &pending_keys);
                    cursor += inserted_len;
                    pending_items.clear();
                    pending_keys.clear();
                }
                new_ranges.insert(
                    key.clone(),
                    GroupRange {
                        start: cursor,
                        len: desired_len,
                    },
                );
                cursor += desired_len;
                pending_start = cursor;
            } else {
                // Rebuild changed groups into a contiguous insertion batch.
                let (items, keys) = self.build_group_block(key, &visible_ids);
                let start = pending_start + pending_items.len();
                let len = items.len();
                new_ranges.insert(key.clone(), GroupRange { start, len });
                pending_items.extend(items);
                pending_keys.extend(keys);
            }
        }

        if !pending_items.is_empty() {
            let _inserted_len = self.insert_block(pending_start, &pending_items, &pending_keys);
        }

        self.group_ranges = new_ranges;
        let mut old_group_order = std::mem::replace(&mut self.group_order, group_order);
        old_group_order.clear();
        self.group_order_scratch = old_group_order;
        self.dirty_groups.clear();

        self.ghost_items
            .retain(|(key, _), _| self.grouped_cache.contains_key(key));

        // Prune interned keys that are no longer referenced by any list state.
        self.interned.retain(|key| Rc::strong_count(key) > 1);

        self.update_empty_overlay();

        // Cross-check the cached grouping against the GTK store after incremental edits.
        let expected_ranges = self
            .group_order
            .iter()
            .filter(|key| {
                self.grouped_cache
                    .get(*key)
                    .map(|ids| !self.visible_ids_for_group(ids).is_empty())
                    .unwrap_or(false)
            })
            .count();
        if self.group_ranges.len() != expected_ranges {
            // Missing ranges leave later stack edits dependent on a full expand/collapse rebuild
            debug!(
                expected_ranges,
                actual_ranges = self.group_ranges.len(),
                "group range mismatch; rebuilding"
            );
            self.rebuild_list();
            return;
        }
        let expected_len = self.expected_list_len();
        let actual_len = self.store.n_items() as usize;
        if actual_len != expected_len {
            // Guard against stale spans or insert/remove drift by forcing a full rebuild.
            debug!(expected_len, actual_len, "list length mismatch; rebuilding");
            self.rebuild_list();
        }
    }

    pub(super) fn request_rebuild(&mut self) {
        self.needs_rebuild = true;
    }

    fn update_empty_overlay(&self) {
        let is_empty = self.store.n_items() == 0;
        // Compare against the widget's own visible flag
        // Effective visibility can flip with parent state and leave the overlay logically stale
        if self.empty_overlay.get_visible() != is_empty {
            self.empty_overlay.set_visible(is_empty);
        }
    }
}
