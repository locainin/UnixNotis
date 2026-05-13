//! Notification list construction and config application
//!
//! This keeps GTK factory setup out of the folder root and separate from list mutation code

use std::rc::Rc;

use async_channel::Sender;
use gio::prelude::*;
use gtk::prelude::*;
use gtk::Align;
use tokio::sync::mpsc;

use crate::dbus::{UiCommand, UiEvent};

use super::super::icons::IconResolver;
use super::list_item::RowKind;
use super::list_row::empty::{build_empty_row, update_empty_row};
use super::list_widgets::{
    bind_row, ensure_row_widgets, get_row_widgets, set_row_widgets, RowWidgets,
};
use super::types::{NotificationList, NotificationListConfig};

impl NotificationList {
    pub fn new(
        scroller: gtk::ScrolledWindow,
        command_tx: mpsc::Sender<UiCommand>,
        event_tx: Sender<UiEvent>,
        icon_resolver: Rc<IconResolver>,
        config: NotificationListConfig,
    ) -> Self {
        let store = gio::ListStore::new::<super::list_item::RowItem>();
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
        // Offset from the top of the list area to match the reference layout
        empty_overlay.set_margin_top(config.empty_offset_top);
        empty_overlay.set_visible(true);
        overlay.add_overlay(&empty_overlay);

        scroller.set_child(Some(&overlay));

        let command_tx_clone = command_tx.clone();
        let event_tx_clone = event_tx.clone();
        factory.connect_setup(move |_, list_item| {
            let widgets = RowWidgets::new(
                RowKind::Notification,
                command_tx_clone.clone(),
                event_tx_clone.clone(),
            );
            set_row_widgets(list_item, Rc::new(widgets));
        });

        let command_tx_clone = command_tx.clone();
        let event_tx_clone = event_tx.clone();
        let icon_resolver_clone = icon_resolver.clone();
        factory.connect_bind(move |_, list_item| {
            let Some(item) = list_item.item().and_downcast::<super::list_item::RowItem>() else {
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
            // ensure_row_widgets when the row is rebound
        });

        Self {
            store,
            empty_overlay,
            empty_offset_top: config.empty_offset_top,
            empty_text: config.empty_text,
            entries: std::collections::HashMap::new(),
            active_order: std::collections::VecDeque::new(),
            history_order: std::collections::VecDeque::new(),
            group_expanded: std::collections::HashMap::new(),
            group_headers: std::collections::HashMap::new(),
            group_order: Vec::new(),
            group_order_scratch: Vec::new(),
            grouped_cache: std::collections::HashMap::new(),
            group_active_index: std::collections::HashMap::new(),
            group_history_index: std::collections::HashMap::new(),
            group_ranges: std::collections::HashMap::new(),
            interned: std::collections::HashSet::new(),
            current_keys: Vec::new(),
            keys_scratch: Vec::new(),
            items_scratch: Vec::new(),
            objects_scratch: Vec::new(),
            needs_rebuild: false,
            dirty_groups: std::collections::HashSet::new(),
            filter_query: None,
            transient_to_history: config.transient_to_history,
            max_active: config.max_active,
            max_entries: config.max_entries,
        }
    }

    pub fn apply_config(&mut self, config: &NotificationListConfig, has_widgets: bool) {
        // Future close handling should use the latest runtime policy
        self.transient_to_history = config.transient_to_history;
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
            // When widgets are visible, align the empty state beneath them
            if self.empty_overlay.valign() != Align::Start {
                self.empty_overlay.set_valign(Align::Start);
            }
            if self.empty_overlay.margin_top() != self.empty_offset_top {
                self.empty_overlay.set_margin_top(self.empty_offset_top);
            }
        } else {
            // When no widgets are visible, center the empty state in the list area
            if self.empty_overlay.valign() != Align::Center {
                self.empty_overlay.set_valign(Align::Center);
            }
            if self.empty_overlay.margin_top() != 0 {
                self.empty_overlay.set_margin_top(0);
            }
        }
    }
}
