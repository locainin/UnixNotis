//! Row widgets and rendering logic for the notification list
//!
//! Keeps GTK widget creation and updates isolated from list state

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::{Rc, Weak};

use async_channel::Sender;
use gtk::prelude::*;
use gtk::{self};
use tokio::sync::mpsc;
use tracing::debug;

use crate::control::{UiCommand, UiEvent};

use super::item::{RowData, RowItem, RowKind};
use super::row::group::{build_group_row, update_group_row, GroupRowWidgets};
use super::row::notification::{
    build_notification_row, update_notification_row, NotificationRowWidgets,
};
use crate::ui::icons::IconResolver;

/// GTK wrapper widgets for each row type
pub(super) struct RowWidgets {
    kind: RowKind,
    root: gtk::Box,
    group: Option<GroupRowWidgets>,
    notification: Option<NotificationRowWidgets>,
    handler: RefCell<Option<(RowItem, gtk::glib::SignalHandlerId)>>,
    command_tx: mpsc::Sender<UiCommand>,
}

// Thread-local storage replaces glib qdata to keep the codebase free of unsafe blocks
// Weak refs let stale entries be collected without explicit destroy signal handling
// The map key is the raw GObject pointer, scoped to the GTK main thread
thread_local! {
    static ROW_WIDGETS: RefCell<HashMap<*const (), Weak<RowWidgets>>> = RefCell::new(HashMap::new());
}

impl RowWidgets {
    pub(super) fn new(
        kind: RowKind,
        command_tx: mpsc::Sender<UiCommand>,
        event_tx: Sender<UiEvent>,
    ) -> Self {
        match kind {
            RowKind::GroupHeader => Self::new_group(command_tx, event_tx),
            RowKind::Notification => Self::new_notification(command_tx),
        }
    }

    fn new_group(command_tx: mpsc::Sender<UiCommand>, event_tx: Sender<UiEvent>) -> Self {
        let (root, group) = build_group_row(event_tx);

        Self {
            kind: RowKind::GroupHeader,
            root,
            group: Some(group),
            notification: None,
            handler: RefCell::new(None),
            command_tx,
        }
    }

    fn new_notification(command_tx: mpsc::Sender<UiCommand>) -> Self {
        let (root, notification) = build_notification_row(command_tx.clone());

        Self {
            kind: RowKind::Notification,
            root,
            group: None,
            notification: Some(notification),
            handler: RefCell::new(None),
            command_tx,
        }
    }

    fn refresh(&self, data: &RowData, icon_resolver: &IconResolver) {
        match self.kind {
            RowKind::GroupHeader => {
                if let Some(group) = &self.group {
                    update_group_row(group, &self.root, data, icon_resolver);
                }
            }
            RowKind::Notification => {
                if let Some(notification) = &self.notification {
                    update_notification_row(notification, data, icon_resolver, &self.command_tx);
                }
            }
        }
    }

    pub(super) fn unbind(&self) {
        self.disconnect();
    }

    fn disconnect(&self) {
        if let Some((item, handler)) = self.handler.borrow_mut().take() {
            item.disconnect(handler);
        }
    }
}

pub(super) fn ensure_row_widgets(
    item: &gtk::ListItem,
    kind: RowKind,
    command_tx: mpsc::Sender<UiCommand>,
    event_tx: Sender<UiEvent>,
) -> Rc<RowWidgets> {
    if let Some(existing) = get_row_widgets(item) {
        if existing.kind == kind {
            return existing;
        }
    }

    let widgets = Rc::new(RowWidgets::new(kind, command_tx, event_tx));
    set_row_widgets(item, widgets.clone());
    debug!(?kind, "row widgets created");
    widgets
}

pub(super) fn bind_row(
    widgets: Rc<RowWidgets>,
    item: &RowItem,
    data: &RowData,
    icon_resolver: Rc<IconResolver>,
) {
    widgets.disconnect();
    widgets.refresh(data, &icon_resolver);
    let item_clone = item.clone();
    let widgets_clone = widgets.clone();
    let icon_resolver = icon_resolver;
    let handler = item.connect_local("updated", false, move |_| {
        let data = item_clone.data();
        widgets_clone.refresh(&data, &icon_resolver);
        None
    });
    *widgets.handler.borrow_mut() = Some((item.clone(), handler));
}

pub(super) fn set_row_widgets(item: &gtk::ListItem, widgets: Rc<RowWidgets>) {
    // Attach the actual row root whenever the cached widget bundle changes
    // Setup also uses this so GTK never keeps an empty placeholder child
    item.set_child(Some(&widgets.root));
    // Store a weak reference in thread-local storage so get_row_widgets can retrieve
    // the cached bundle without holding any Rc strong count from the map
    ROW_WIDGETS.with(|map| {
        map.borrow_mut()
            .insert(glib_ptr(item), Rc::downgrade(&widgets));
    });
}

pub(super) fn get_row_widgets(item: &gtk::ListItem) -> Option<Rc<RowWidgets>> {
    // Look up the cached RowWidgets bundle by GObject pointer
    // Stale weak refs (from destroyed or recycled list items) are removed on access
    ROW_WIDGETS.with(|map| {
        let mut map = map.borrow_mut();
        let key = glib_ptr(item);
        match map.get(&key) {
            Some(weak) => weak.upgrade().or_else(|| {
                map.remove(&key);
                None
            }),
            None => None,
        }
    })
}

fn glib_ptr<T: gtk::glib::prelude::ObjectType>(obj: &T) -> *const () {
    // Extract the raw GObject pointer for use as a thread-local HashMap key
    // This replaces glib qdata with safe Rust storage while preserving identity
    obj.as_ptr() as *const ()
}

#[cfg(test)]
#[path = "tests/widgets.rs"]
mod tests;
