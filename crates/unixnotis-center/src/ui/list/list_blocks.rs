//! Notification list block assembly and list-store mutation helpers.

use std::rc::Rc;

use gtk::glib;
use gtk::glib::object::Cast;

use super::list_item::RowData;
use super::{NotificationList, RowItem, RowKey};

impl NotificationList {
    pub(super) fn build_group_block(
        &mut self,
        key: &Rc<str>,
        ids: &[u32],
    ) -> (Vec<RowItem>, Vec<RowKey>) {
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

    pub(super) fn group_block_len(&self, key: &Rc<str>, ids: &[u32]) -> usize {
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

    pub(super) fn remove_block(&mut self, start: usize, len: usize) {
        if len == 0 {
            return;
        }
        self.store
            .splice(start as u32, len as u32, &[] as &[glib::Object]);
        self.current_keys.drain(start..start + len);
        self.shift_group_ranges(start, -(len as isize), false);
    }

    pub(super) fn insert_block(
        &mut self,
        start: usize,
        items: &[RowItem],
        keys: &[RowKey],
    ) -> usize {
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

    pub(super) fn shift_group_ranges(&mut self, start: usize, delta: isize, inclusive: bool) {
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
}

pub(super) fn common_prefix_suffix(current: &[RowKey], next: &[RowKey]) -> (usize, usize) {
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
