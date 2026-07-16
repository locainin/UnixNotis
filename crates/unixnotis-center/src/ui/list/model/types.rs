//! Core list types shared by the list submodules
//!
//! This keeps state definitions out of `mod.rs` so the folder root stays wiring-only

use std::collections::{HashMap, HashSet, VecDeque};
use std::rc::Rc;

use gtk::glib;
use unixnotis_core::EmptyStateAlignment;
use unixnotis_core::NotificationView;

use super::item::RowItem;

/// Maintains notification data and renders grouped widgets into the panel list
pub struct NotificationList {
    pub(in crate::ui::list) store: gio::ListStore,
    pub(in crate::ui) empty_overlay: gtk::Box,
    pub(in crate::ui::list) empty_offset_top: i32,
    pub(in crate::ui::list) empty_alignment: EmptyStateAlignment,
    pub(in crate::ui) empty_text: String,
    pub(in crate::ui::list) entries: HashMap<u32, NotificationEntry>,
    // Active notifications render first to match the in-flight stack
    pub(in crate::ui::list) active_order: VecDeque<u32>,
    // Historical notifications follow active ones in most-recent-first order
    pub(in crate::ui::list) history_order: VecDeque<u32>,
    pub(in crate::ui::list) group_expanded: HashMap<Rc<str>, bool>,
    pub(in crate::ui::list) group_headers: HashMap<Rc<str>, RowItem>,
    pub(in crate::ui::list) group_order: Vec<Rc<str>>,
    pub(in crate::ui::list) group_order_scratch: Vec<Rc<str>>,
    pub(in crate::ui::list) grouped_cache: HashMap<Rc<str>, Vec<u32>>,
    // Incremental per-group indices keep regrouping costs local to changed ids
    pub(in crate::ui::list) group_active_index: HashMap<Rc<str>, VecDeque<u32>>,
    pub(in crate::ui::list) group_history_index: HashMap<Rc<str>, VecDeque<u32>>,
    // Tracks the row span for each group to support incremental list updates
    pub(in crate::ui::list) group_ranges: HashMap<Rc<str>, GroupRange>,
    pub(in crate::ui::list) interned: HashSet<Rc<str>>,
    pub(in crate::ui::list) current_keys: Vec<RowKey>,
    pub(in crate::ui::list) keys_scratch: Vec<RowKey>,
    pub(in crate::ui::list) items_scratch: Vec<RowItem>,
    pub(in crate::ui::list) objects_scratch: Vec<glib::Object>,
    pub(in crate::ui::list) needs_rebuild: bool,
    // Groups with pending content or visibility changes since the last flush
    pub(in crate::ui::list) dirty_groups: HashSet<Rc<str>>,
    // Normalized filter query for notification search in the panel header
    pub(in crate::ui::list) filter_query: Option<FilterQuery>,
    // Local close handling needs the same transient history rule as the daemon
    pub(in crate::ui::list) transient_to_history: bool,
    // Optional metadata lanes stay config-owned so the stock row remains compact
    pub(in crate::ui::list) show_notification_metadata: bool,
    pub(in crate::ui::list) show_notification_thumbnails: bool,
    pub(in crate::ui::list) max_active: usize,
    pub(in crate::ui::list) max_entries: usize,
}

/// Input settings that influence list rendering and empty-state behavior
pub struct NotificationListConfig {
    pub max_active: usize,
    pub max_entries: usize,
    pub transient_to_history: bool,
    pub show_notification_metadata: bool,
    pub show_notification_thumbnails: bool,
    pub empty_text: String,
    pub empty_offset_top: i32,
    pub empty_alignment: EmptyStateAlignment,
}

pub(in crate::ui::list) struct NotificationEntry {
    pub(in crate::ui::list) view: Rc<NotificationView>,
    pub(in crate::ui::list) is_active: bool,
    pub(in crate::ui::list) received_at_ms: i64,
    pub(in crate::ui::list) app_key: Rc<str>,
    pub(in crate::ui::list) item: RowItem,
}

#[derive(Clone, Copy, Debug)]
pub(in crate::ui::list) struct GroupRange {
    pub(in crate::ui::list) start: usize,
    pub(in crate::ui::list) len: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::ui::list) struct FilterQuery {
    // Keep the normalized text compact because this value is cloned and compared
    pub(in crate::ui::list) text: Box<str>,
    // ASCII queries can use a byte-wise fast path without allocating a lowered haystack
    pub(in crate::ui::list) ascii_only: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(in crate::ui::list) enum RowKey {
    GroupHeader { group: Rc<str> },
    Notification { id: u32 },
}

#[cfg(test)]
#[path = "tests/types.rs"]
mod tests;
