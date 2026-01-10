//! Notification list state and rendering wiring.
//!
//! Keeps list bookkeeping in this module while delegating row widgets to
//! `list_widgets.rs` to avoid bloating unrelated logic.

#[path = "list_widgets.rs"]
mod list_widgets;

use std::collections::{HashMap, HashSet, VecDeque};
use std::rc::Rc;

use async_channel::Sender;
use gio::prelude::*;
use gtk::glib;
use gtk::prelude::*;
use tokio::sync::mpsc::UnboundedSender;
use tracing::debug;
use unixnotis_core::{CloseReason, NotificationView};

use crate::dbus::{UiCommand, UiEvent};

use super::icons::IconResolver;
use super::list_item::{RowData, RowItem, RowKind};
use list_widgets::{
    bind_row, clear_row_widgets, ensure_row_widgets, get_row_widgets, set_row_widgets, RowWidgets,
};

/// Maintains notification data and renders grouped widgets into the panel list.
pub struct NotificationList {
    store: gio::ListStore,
    entries: HashMap<u32, NotificationEntry>,
    // Active notifications render first to match the in-flight stack.
    active_order: VecDeque<u32>,
    // Historical notifications follow active ones in most-recent-first order.
    history_order: VecDeque<u32>,
    group_expanded: HashMap<Rc<str>, bool>,
    group_headers: HashMap<Rc<str>, RowItem>,
    group_order: Vec<Rc<str>>,
    group_order_scratch: Vec<Rc<str>>,
    grouped_cache: HashMap<Rc<str>, Vec<u32>>,
    // Tracks the row span for each group to support incremental list updates.
    group_ranges: HashMap<Rc<str>, GroupRange>,
    ghost_items: HashMap<(Rc<str>, u8), RowItem>,
    interned: HashSet<Rc<str>>,
    current_keys: Vec<RowKey>,
    keys_scratch: Vec<RowKey>,
    items_scratch: Vec<RowItem>,
    objects_scratch: Vec<glib::Object>,
    needs_rebuild: bool,
    // Groups with pending content/visibility changes since the last flush.
    dirty_groups: HashSet<Rc<str>>,
    max_active: usize,
    max_entries: usize,
}

struct NotificationEntry {
    view: Rc<NotificationView>,
    is_active: bool,
    app_key: Rc<str>,
    item: RowItem,
}

#[derive(Clone, Copy, Debug)]
struct GroupRange {
    start: usize,
    len: usize,
}

impl NotificationList {
    pub fn new(
        scroller: gtk::ScrolledWindow,
        command_tx: UnboundedSender<UiCommand>,
        event_tx: Sender<UiEvent>,
        icon_resolver: Rc<IconResolver>,
        max_active: usize,
        max_entries: usize,
    ) -> Self {
        let store = gio::ListStore::new::<RowItem>();
        let selection = gtk::NoSelection::new(Some(store.clone()));
        let factory = gtk::SignalListItemFactory::new();

        let list_view = gtk::ListView::new(Some(selection), Some(factory.clone()));
        list_view.add_css_class("unixnotis-panel-list");
        list_view.set_hexpand(true);
        list_view.set_vexpand(true);

        scroller.set_child(Some(&list_view));

        let command_tx_clone = command_tx.clone();
        let event_tx_clone = event_tx.clone();
        factory.connect_setup(move |_, list_item| {
            let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
            list_item.set_child(Some(&root));

            let widgets = RowWidgets::new(
                RowKind::Ghost,
                command_tx_clone.clone(),
                event_tx_clone.clone(),
            );
            set_row_widgets(list_item, Rc::new(widgets));
        });

        let command_tx_clone = command_tx.clone();
        let event_tx_clone = event_tx.clone();
        let icon_resolver_clone = icon_resolver.clone();
        factory.connect_bind(move |_, list_item| {
            let Some(item) = list_item.item().and_downcast::<RowItem>() else {
                return;
            };
            let data = item.data();
            let widgets = ensure_row_widgets(
                list_item,
                data.kind,
                command_tx_clone.clone(),
                event_tx_clone.clone(),
            );

            bind_row(widgets, &item, &data, icon_resolver_clone.clone());
        });

        factory.connect_unbind(move |_, list_item| {
            if let Some(widgets) = get_row_widgets(list_item) {
                widgets.unbind();
            }
            clear_row_widgets(list_item);
        });

        Self {
            store,
            entries: HashMap::new(),
            active_order: VecDeque::new(),
            history_order: VecDeque::new(),
            group_expanded: HashMap::new(),
            group_headers: HashMap::new(),
            group_order: Vec::new(),
            group_order_scratch: Vec::new(),
            grouped_cache: HashMap::new(),
            group_ranges: HashMap::new(),
            ghost_items: HashMap::new(),
            interned: HashSet::new(),
            current_keys: Vec::new(),
            keys_scratch: Vec::new(),
            items_scratch: Vec::new(),
            objects_scratch: Vec::new(),
            needs_rebuild: false,
            dirty_groups: HashSet::new(),
            max_active,
            max_entries,
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

    fn rebuild_list(&mut self) {
        let mut group_order = std::mem::take(&mut self.group_order_scratch);
        group_order.clear();
        let mut grouped = std::mem::take(&mut self.grouped_cache);
        grouped.clear();

        // Build app-based groups in active+history order for stable UI layout.
        for id in self.active_order.iter().chain(self.history_order.iter()) {
            let Some(entry) = self.entries.get(id) else {
                continue;
            };
            let key = entry.app_key.clone();
            let bucket = grouped.entry(key.clone()).or_insert_with(|| {
                group_order.push(key.clone());
                Vec::new()
            });
            bucket.push(*id);
        }

        self.group_headers
            .retain(|key, _| grouped.contains_key(key));
        self.group_expanded
            .retain(|key, _| grouped.contains_key(key));

        let mut items = std::mem::take(&mut self.items_scratch);
        items.clear();
        let mut keys = std::mem::take(&mut self.keys_scratch);
        keys.clear();
        let mut group_ranges = HashMap::new();
        for key in &group_order {
            let Some(ids) = grouped.get(key) else {
                continue;
            };
            let start = items.len();
            let (block_items, block_keys) = self.build_group_block(key, ids);
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
        let (prefix, suffix) = common_prefix_suffix(&current_keys, &keys);
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

        let group_count = grouped.len();
        self.grouped_cache = grouped;
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
        group_order.clear();
        let mut grouped = std::mem::take(&mut self.grouped_cache);
        grouped.clear();

        for id in self.active_order.iter().chain(self.history_order.iter()) {
            let Some(entry) = self.entries.get(id) else {
                continue;
            };
            let key = entry.app_key.clone();
            let bucket = grouped.entry(key.clone()).or_insert_with(|| {
                group_order.push(key.clone());
                Vec::new()
            });
            bucket.push(*id);
        }

        self.group_headers
            .retain(|key, _| grouped.contains_key(key));
        self.group_expanded
            .retain(|key, _| grouped.contains_key(key));

        let mut keep_groups: HashSet<Rc<str>> = HashSet::new();
        let mut removed_groups: HashSet<Rc<str>> = HashSet::new();
        let mut remove_ranges: Vec<GroupRange> = Vec::new();
        for (key, range) in self.group_ranges.iter() {
            let Some(ids) = grouped.get(key) else {
                remove_ranges.push(*range);
                removed_groups.insert(key.clone());
                continue;
            };
            let desired_len = self.group_block_len(key, ids);
            if !self.dirty_groups.contains(key) && range.len == desired_len {
                keep_groups.insert(key.clone());
            } else {
                remove_ranges.push(*range);
                removed_groups.insert(key.clone());
            }
        }

        remove_ranges.sort_by_key(|range| range.start);
        let mut merged: Vec<GroupRange> = Vec::new();
        for range in remove_ranges {
            if let Some(last) = merged.last_mut() {
                if last.start + last.len == range.start {
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
            let Some(ids) = grouped.get(key) else {
                continue;
            };
            let desired_len = self.group_block_len(key, ids);
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
                let (items, keys) = self.build_group_block(key, ids);
                pending_items.extend(items);
                pending_keys.extend(keys);
            }
        }

        if !pending_items.is_empty() {
            let _inserted_len = self.insert_block(pending_start, &pending_items, &pending_keys);
        }

        self.group_ranges = new_ranges;
        self.grouped_cache = grouped;
        let mut old_group_order = std::mem::replace(&mut self.group_order, group_order);
        old_group_order.clear();
        self.group_order_scratch = old_group_order;
        self.dirty_groups.clear();

        self.ghost_items
            .retain(|(key, _), _| self.grouped_cache.contains_key(key));

        // Prune interned keys that are no longer referenced by any list state.
        self.interned.retain(|key| Rc::strong_count(key) > 1);
    }

    fn build_group_block(&mut self, key: &Rc<str>, ids: &[u32]) -> (Vec<RowItem>, Vec<RowKey>) {
        let expanded = self.group_expanded.get(key).copied().unwrap_or(false);
        let Some(first_entry) = ids.first().and_then(|id| self.entries.get(id)) else {
            return (Vec::new(), Vec::new());
        };

        let header = self.group_headers.entry(key.clone()).or_insert_with(|| {
            RowItem::new(RowData::group_header(
                key.clone(),
                ids.len(),
                expanded,
                first_entry.view.clone(),
            ))
        });
        header.update(RowData::group_header(
            key.clone(),
            ids.len(),
            expanded,
            first_entry.view.clone(),
        ));

        let mut items = Vec::new();
        let mut keys = Vec::new();
        items.push(header.clone());
        keys.push(RowKey::GroupHeader { group: key.clone() });

        let stacked = !expanded && ids.len() > 1;
        for (index, id) in ids.iter().enumerate() {
            if !expanded && index > 0 {
                break;
            }
            let Some(entry) = self.entries.get(id) else {
                continue;
            };
            entry.item.update(RowData::notification(
                entry.app_key.clone(),
                entry.view.clone(),
                stacked,
                entry.is_active,
            ));
            items.push(entry.item.clone());
            keys.push(RowKey::Notification { id: *id });
        }

        if stacked {
            let ghost_count = ids.len().saturating_sub(1).min(2);
            for depth in 1..=ghost_count {
                let ghost_key = (key.clone(), depth as u8);
                let ghost = self
                    .ghost_items
                    .entry(ghost_key)
                    .or_insert_with(|| RowItem::new(RowData::ghost(key.clone(), depth as u8)));
                ghost.update(RowData::ghost(key.clone(), depth as u8));
                items.push(ghost.clone());
                keys.push(RowKey::Ghost {
                    group: key.clone(),
                    depth: depth as u8,
                });
            }
        }

        (items, keys)
    }

    fn group_block_len(&self, key: &Rc<str>, ids: &[u32]) -> usize {
        let expanded = self.group_expanded.get(key).copied().unwrap_or(false);
        let mut len = 1; // header
        if expanded {
            len += ids.len();
        } else if !ids.is_empty() {
            len += 1;
        }
        if !expanded && ids.len() > 1 {
            len += ids.len().saturating_sub(1).min(2);
        }
        len
    }

    fn remove_block(&mut self, start: usize, len: usize) {
        if len == 0 {
            return;
        }
        self.store
            .splice(start as u32, len as u32, &[] as &[glib::Object]);
        self.current_keys.drain(start..start + len);
        self.shift_group_ranges(start, -(len as isize), false);
    }

    fn insert_block(&mut self, start: usize, items: &[RowItem], keys: &[RowKey]) -> usize {
        if items.is_empty() {
            return 0;
        }
        let mut objects = std::mem::take(&mut self.objects_scratch);
        objects.clear();
        for item in items {
            objects.push(item.clone().upcast::<glib::Object>());
        }
        self.store.splice(start as u32, 0, &objects);
        self.current_keys.splice(start..start, keys.iter().cloned());
        self.shift_group_ranges(start, items.len() as isize, true);
        objects.clear();
        self.objects_scratch = objects;
        items.len()
    }

    fn shift_group_ranges(&mut self, start: usize, delta: isize, inclusive: bool) {
        if delta == 0 {
            return;
        }
        for range in self.group_ranges.values_mut() {
            let should_shift = if inclusive {
                range.start >= start
            } else {
                range.start > start
            };
            if should_shift {
                range.start = (range.start as isize + delta) as usize;
            }
        }
    }

    fn intern_key(&mut self, key: &str) -> Rc<str> {
        if let Some(value) = self.interned.get(key) {
            return value.clone();
        }
        // Intern group keys to keep row identity stable across updates.
        let value: Rc<str> = Rc::from(key);
        self.interned.insert(value.clone());
        value
    }

    fn request_rebuild(&mut self) {
        self.needs_rebuild = true;
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum RowKey {
    GroupHeader { group: Rc<str> },
    Notification { id: u32 },
    Ghost { group: Rc<str>, depth: u8 },
}

fn common_prefix_suffix(current: &[RowKey], next: &[RowKey]) -> (usize, usize) {
    let mut prefix = 0;
    let min_len = current.len().min(next.len());
    while prefix < min_len && current[prefix] == next[prefix] {
        prefix += 1;
    }

    let mut suffix = 0;
    while suffix < current.len().saturating_sub(prefix)
        && suffix < next.len().saturating_sub(prefix)
        && current[current.len() - 1 - suffix] == next[next.len() - 1 - suffix]
    {
        suffix += 1;
    }

    (prefix, suffix)
}
