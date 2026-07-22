//! Notification list construction and config application
//!
//! This keeps GTK factory setup out of the folder root and separate from list mutation code

use std::rc::Rc;

use async_channel::Sender;
use gio::prelude::*;
use gtk::prelude::*;
use gtk::Align;
use tokio::sync::mpsc;

use crate::control::{UiCommand, UiEvent};

use super::item::RowKind;
use super::row::empty::build_empty_row;
use super::types::{NotificationList, NotificationListConfig};
use super::widgets::{bind_row, ensure_row_widgets, get_row_widgets, set_row_widgets, RowWidgets};
use crate::ui::icons::IconResolver;

impl NotificationList {
    pub fn new(
        scroller: gtk::ScrolledWindow,
        command_tx: mpsc::Sender<UiCommand>,
        event_tx: Sender<UiEvent>,
        icon_resolver: Rc<IconResolver>,
        config: NotificationListConfig,
    ) -> Self {
        let store = gio::ListStore::new::<super::item::RowItem>();
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
        factory.connect_setup(move |_, item| {
            let Some(gtk_item) = item.downcast_ref::<gtk::ListItem>() else {
                return;
            };
            let widgets = RowWidgets::new(
                RowKind::Notification,
                command_tx_clone.clone(),
                event_tx_clone.clone(),
            );
            set_row_widgets(gtk_item, Rc::new(widgets));
        });

        let command_tx_clone = command_tx;
        let event_tx_clone = event_tx;
        let icon_resolver_clone = icon_resolver;
        factory.connect_bind(move |_, item| {
            let Some(gtk_item) = item.downcast_ref::<gtk::ListItem>() else {
                return;
            };
            let Some(row_item) = gtk_item.item().and_downcast::<super::item::RowItem>() else {
                return;
            };
            let data = row_item.data();
            let widgets = ensure_row_widgets(
                gtk_item,
                data.kind,
                command_tx_clone.clone(),
                event_tx_clone.clone(),
            );

            bind_row(widgets, &row_item, &data, icon_resolver_clone.clone());
        });

        factory.connect_unbind(move |_, item| {
            let Some(gtk_item) = item.downcast_ref::<gtk::ListItem>() else {
                return;
            };
            if let Some(widgets) = get_row_widgets(gtk_item) {
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
            empty_alignment: config.empty_alignment,
            empty_text: config.empty_text,
            no_matching_text: config.no_matching_text,
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
            show_notification_metadata: config.show_notification_metadata,
            notification_metadata: Rc::new(config.notification_metadata),
            notification_corners: config.notification_corners,
            show_notification_thumbnails: config.show_notification_thumbnails,
            reduced_motion: config.reduced_motion,
            max_active: config.max_active,
            max_entries: config.max_entries,
        }
    }

    pub fn apply_config(&mut self, config: &NotificationListConfig) {
        // Future close handling should use the latest runtime policy
        self.transient_to_history = config.transient_to_history;
        let presentation_changed = self.show_notification_metadata
            != config.show_notification_metadata
            || self.notification_metadata.as_ref() != &config.notification_metadata
            || self.notification_corners != config.notification_corners
            || self.show_notification_thumbnails != config.show_notification_thumbnails
            || self.reduced_motion != config.reduced_motion;
        self.show_notification_metadata = config.show_notification_metadata;
        if self.notification_metadata.as_ref() != &config.notification_metadata {
            self.notification_metadata = Rc::new(config.notification_metadata.clone());
        }
        self.notification_corners = config.notification_corners;
        self.show_notification_thumbnails = config.show_notification_thumbnails;
        self.reduced_motion = config.reduced_motion;
        if self.empty_text != config.empty_text {
            self.empty_text = config.empty_text.clone();
        }
        if self.no_matching_text != config.no_matching_text {
            self.no_matching_text = config.no_matching_text.clone();
        }
        // The visible copy depends on both configuration and current search state
        self.update_empty_overlay();
        if self.empty_offset_top != config.empty_offset_top {
            self.empty_offset_top = config.empty_offset_top;
        }
        self.empty_alignment = config.empty_alignment;
        self.apply_limits(config.max_active, config.max_entries);
        if presentation_changed {
            // Existing rows need fresh RowData so optional lanes hide or show immediately
            self.dirty_groups.extend(self.grouped_cache.keys().cloned());
            self.request_rebuild();
        }
    }

    pub fn set_empty_layout(&self, has_widgets: bool) {
        let alignment = resolve_empty_alignment(self.empty_alignment, has_widgets);
        if self.empty_overlay.valign() != alignment {
            self.empty_overlay.set_valign(alignment);
        }
        // A top offset is meaningful only when the label starts at the top edge
        let margin_top = if alignment == Align::Start {
            self.empty_offset_top
        } else {
            0
        };
        if self.empty_overlay.margin_top() != margin_top {
            self.empty_overlay.set_margin_top(margin_top);
        }
    }
}

const fn resolve_empty_alignment(
    alignment: unixnotis_core::EmptyStateAlignment,
    has_widgets: bool,
) -> Align {
    use unixnotis_core::EmptyStateAlignment;

    match alignment {
        EmptyStateAlignment::Auto if has_widgets => Align::Start,
        EmptyStateAlignment::Auto | EmptyStateAlignment::Center => Align::Center,
        EmptyStateAlignment::Start => Align::Start,
        EmptyStateAlignment::End => Align::End,
    }
}

#[cfg(test)]
#[path = "tests/build.rs"]
mod tests;
