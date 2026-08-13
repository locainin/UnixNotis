//! Notification list block assembly and list-store mutation helpers

use std::rc::Rc;

use gtk::glib;
use gtk::glib::object::Cast;

use super::item::{RowData, RowPresentation};
use super::types::{NotificationList, RowKey};
use super::RowItem;

impl NotificationList {
    pub(in crate::ui::notifications) fn build_group_block(
        &mut self,
        key: &Rc<str>,
        ids: &[u32],
    ) -> (Vec<RowItem>, Vec<RowKey>) {
        let expanded = self.group_expanded.get(key).copied().unwrap_or(false);
        let Some(first_entry) = ids.first().and_then(|id| self.entries.get(id)) else {
            return (Vec::new(), Vec::new());
        };

        let mut items = Vec::new();
        let mut keys = Vec::new();
        // Every application block owns one shared identity header
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
        items.push(header.clone());
        keys.push(RowKey::GroupHeader { group: key.clone() });

        // Collapsed groups render the newest content row under their shared header
        let collapsed_group_preview = !expanded && ids.len() > 1;
        let stack_depth = collapsed_stack_depth(ids.len(), expanded);
        for (index, id) in ids.iter().enumerate() {
            if !expanded && index > 0 {
                break;
            }
            let Some(entry) = self.entries.get(id) else {
                continue;
            };
            let presentation = RowPresentation {
                received_at_ms: entry.received_at_ms,
                show_metadata: self.show_notification_metadata,
                show_thumbnail: self.show_notification_thumbnails,
                show_avatar: self.show_notification_avatars,
                reduced_motion: self.reduced_motion,
                metadata: self.notification_metadata.clone(),
                card_corners: self.notification_corners,
            };
            let row = RowData::notification(
                entry.app_key.clone(),
                entry.view.clone(),
                collapsed_group_preview,
                stack_depth,
                expanded,
                entry.is_active,
                presentation,
            );
            entry.item.update(row);
            items.push(entry.item.clone());
            keys.push(RowKey::Notification { id: *id });
        }

        (items, keys)
    }

    pub(in crate::ui::notifications) fn group_block_len(
        &self,
        key: &Rc<str>,
        ids: &[u32],
    ) -> usize {
        let expanded = self.group_expanded.get(key).copied().unwrap_or(false);
        if ids.is_empty() {
            return 0;
        }
        let mut len = 1; // shared header
        if expanded {
            len += ids.len();
        } else if !ids.is_empty() {
            len += 1;
        }
        len
    }

    pub(in crate::ui::notifications) fn remove_block(&mut self, start: usize, len: usize) {
        if len == 0 {
            return;
        }
        // Store and key vectors must change in the same window
        self.store
            .splice(start as u32, len as u32, &[] as &[glib::Object]);
        self.current_keys.drain(start..start + len);
        self.shift_group_ranges(start, -(len as isize), false);
    }

    pub(in crate::ui::notifications) fn insert_block(
        &mut self,
        start: usize,
        items: &[RowItem],
        keys: &[RowKey],
    ) -> usize {
        if items.is_empty() {
            return 0;
        }
        // Reuse the conversion buffer to avoid allocating during notification bursts
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

    pub(in crate::ui::notifications) fn shift_group_ranges(
        &mut self,
        start: usize,
        delta: isize,
        inclusive: bool,
    ) {
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

pub(in crate::ui::notifications) fn collapsed_stack_depth(count: usize, expanded: bool) -> u8 {
    if expanded {
        return 0;
    }
    // One hidden item adds one layer and larger groups cap at two quiet silhouettes
    count.saturating_sub(1).min(2) as u8
}

pub(in crate::ui::notifications) fn common_prefix_suffix(
    current: &[RowKey],
    next: &[RowKey],
) -> (usize, usize) {
    // Compute shared prefix and suffix so splices touch only the changed window
    let prefix = current
        .iter()
        .zip(next.iter())
        .take_while(|(left, right)| left == right)
        .count();

    let suffix_limit = current.len().min(next.len()).saturating_sub(prefix);
    let suffix = current
        .iter()
        .rev()
        .zip(next.iter().rev())
        .take(suffix_limit)
        .take_while(|(left, right)| left == right)
        .count();

    (prefix, suffix)
}

#[cfg(test)]
#[path = "tests/blocks.rs"]
mod tests;
