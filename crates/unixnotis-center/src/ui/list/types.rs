//! Core list types shared by the list submodules
//!
//! This keeps state definitions out of `mod.rs` so the folder root stays wiring-only

use std::collections::{HashMap, HashSet, VecDeque};
use std::rc::Rc;

use gio;
use gtk;
use gtk::glib;
use unixnotis_core::NotificationView;

use super::list_item::RowItem;

/// Maintains notification data and renders grouped widgets into the panel list.
pub struct NotificationList {
    pub(super) store: gio::ListStore,
    pub(super) empty_overlay: gtk::Box,
    pub(super) empty_offset_top: i32,
    pub(super) empty_text: String,
    pub(super) entries: HashMap<u32, NotificationEntry>,
    // Active notifications render first to match the in-flight stack
    pub(super) active_order: VecDeque<u32>,
    // Historical notifications follow active ones in most-recent-first order
    pub(super) history_order: VecDeque<u32>,
    pub(super) group_expanded: HashMap<Rc<str>, bool>,
    pub(super) group_headers: HashMap<Rc<str>, RowItem>,
    pub(super) group_order: Vec<Rc<str>>,
    pub(super) group_order_scratch: Vec<Rc<str>>,
    pub(super) grouped_cache: HashMap<Rc<str>, Vec<u32>>,
    // Incremental per-group indices keep regrouping costs local to changed ids
    pub(super) group_active_index: HashMap<Rc<str>, VecDeque<u32>>,
    pub(super) group_history_index: HashMap<Rc<str>, VecDeque<u32>>,
    // Tracks the row span for each group to support incremental list updates
    pub(super) group_ranges: HashMap<Rc<str>, GroupRange>,
    pub(super) ghost_items: HashMap<(Rc<str>, u8), RowItem>,
    pub(super) interned: HashSet<Rc<str>>,
    pub(super) current_keys: Vec<RowKey>,
    pub(super) keys_scratch: Vec<RowKey>,
    pub(super) items_scratch: Vec<RowItem>,
    pub(super) objects_scratch: Vec<glib::Object>,
    pub(super) needs_rebuild: bool,
    // Groups with pending content or visibility changes since the last flush
    pub(super) dirty_groups: HashSet<Rc<str>>,
    // Normalized filter query for notification search in the panel header
    pub(super) filter_query: Option<FilterQuery>,
    // Local close handling needs the same transient history rule as the daemon
    pub(super) transient_to_history: bool,
    pub(super) max_active: usize,
    pub(super) max_entries: usize,
}

/// Input settings that influence list rendering and empty-state behavior.
pub struct NotificationListConfig {
    pub max_active: usize,
    pub max_entries: usize,
    pub transient_to_history: bool,
    pub empty_text: String,
    pub empty_offset_top: i32,
}

pub(super) struct NotificationEntry {
    pub(super) view: Rc<NotificationView>,
    pub(super) is_active: bool,
    pub(super) app_key: Rc<str>,
    pub(super) item: RowItem,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct GroupRange {
    pub(super) start: usize,
    pub(super) len: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct FilterQuery {
    // Keep the normalized text compact because this value is cloned and compared
    pub(super) text: Box<str>,
    // ASCII queries can use a byte-wise fast path without allocating a lowered haystack
    pub(super) ascii_only: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) enum RowKey {
    GroupHeader { group: Rc<str> },
    Notification { id: u32 },
    Ghost { group: Rc<str>, depth: u8 },
}
