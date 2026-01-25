//! Notification list state and rendering wiring.
//!
//! Keeps list bookkeeping in this module while delegating row widgets to
//! `list_widgets.rs` to avoid bloating unrelated logic.

mod list_blocks;
mod list_grouping;
mod list_item;
mod list_row_empty;
mod list_row_ghost;
mod list_row_group;
mod list_row_notification;
mod list_state;
mod list_update;
mod list_widgets;

use std::collections::{HashMap, HashSet, VecDeque};
use std::rc::Rc;

use async_channel::Sender;
use gio::prelude::*;
use gtk::glib;
use gtk::prelude::*;
use gtk::Align;
use tokio::sync::mpsc;
use unixnotis_core::NotificationView;

use crate::dbus::{UiCommand, UiEvent};

use self::list_item::{RowItem, RowKind};
use self::list_row_empty::{build_empty_row, update_empty_row};
use self::list_widgets::{
    bind_row, ensure_row_widgets, get_row_widgets, set_row_widgets, RowWidgets,
};
use super::icons::IconResolver;

/// Maintains notification data and renders grouped widgets into the panel list.
pub struct NotificationList {
    store: gio::ListStore,
    empty_overlay: gtk::Box,
    empty_offset_top: i32,
    empty_text: String,
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

/// Input settings that influence list rendering and empty-state behavior.
pub struct NotificationListConfig {
    pub max_active: usize,
    pub max_entries: usize,
    pub empty_text: String,
    pub empty_offset_top: i32,
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
        command_tx: mpsc::Sender<UiCommand>,
        event_tx: Sender<UiEvent>,
        icon_resolver: Rc<IconResolver>,
        config: NotificationListConfig,
    ) -> Self {
        let store = gio::ListStore::new::<RowItem>();
        let selection = gtk::NoSelection::new(Some(store.clone()));
        let factory = gtk::SignalListItemFactory::new();

        let list_view = gtk::ListView::new(Some(selection), Some(factory.clone()));
        list_view.add_css_class("unixnotis-panel-list");
        list_view.set_hexpand(true);
        list_view.set_vexpand(true);

        let overlay = gtk::Overlay::new();
        overlay.add_css_class("unixnotis-panel-list-overlay");
        overlay.set_child(Some(&list_view));

        let empty_overlay = build_empty_row(&config.empty_text);
        empty_overlay.set_halign(Align::Center);
        empty_overlay.set_valign(Align::Start);
        empty_overlay.set_hexpand(true);
        empty_overlay.set_vexpand(true);
        // Offset from the top of the list area to match the reference layout.
        empty_overlay.set_margin_top(config.empty_offset_top);
        empty_overlay.set_visible(true);
        overlay.add_overlay(&empty_overlay);

        scroller.set_child(Some(&overlay));

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
            // Keep RowWidgets attached so GTK can recycle rows without rebuilding
            // the widget tree on every scroll. Kind mismatches are handled in
            // ensure_row_widgets when the row is rebound.
        });

        Self {
            store,
            empty_overlay,
            empty_offset_top: config.empty_offset_top,
            empty_text: config.empty_text,
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
            max_active: config.max_active,
            max_entries: config.max_entries,
        }
    }

    pub fn apply_config(&mut self, config: &NotificationListConfig, has_widgets: bool) {
        if self.empty_text != config.empty_text {
            update_empty_row(&self.empty_overlay, &config.empty_text);
            self.empty_text = config.empty_text.clone();
        }
        if self.empty_offset_top != config.empty_offset_top {
            self.empty_offset_top = config.empty_offset_top;
        }
        self.set_empty_layout(has_widgets);
        self.apply_limits(config.max_active, config.max_entries);
    }

    pub fn set_empty_layout(&self, has_widgets: bool) {
        if has_widgets {
            // When widgets are visible, align the empty state beneath them.
            self.empty_overlay.set_valign(Align::Start);
            self.empty_overlay.set_margin_top(self.empty_offset_top);
        } else {
            // When no widgets are visible, center the empty state in the list area.
            self.empty_overlay.set_valign(Align::Center);
            self.empty_overlay.set_margin_top(0);
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum RowKey {
    GroupHeader { group: Rc<str> },
    Notification { id: u32 },
    Ghost { group: Rc<str>, depth: u8 },
}
