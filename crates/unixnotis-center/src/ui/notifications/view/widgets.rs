//! Row widgets and rendering logic for the notification list
//!
//! Keeps GTK widget creation and updates isolated from list state

use std::cell::RefCell;
use std::rc::Rc;

use async_channel::Sender;
use gtk::prelude::*;
use gtk::{self};
use tokio::sync::mpsc;
use tracing::debug;

use crate::control::{UiCommand, UiEvent};

use super::item::{RowData, RowItem, RowKind};
use super::row::group::{build_group_row, update_group_row, GroupRowWidgets};
use super::row::notification::{
    build_notification_row, clear_notification_row, update_notification_row, NotificationRowWidgets,
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

// Weak item references prevent destroyed list items from keeping entries alive
// Strong widget bundles preserve the factory's reusable row tree between binds
const MAX_TRACKED_ROW_WIDGETS: usize = 4096;

thread_local! {
    static ROW_WIDGETS: RefCell<Vec<(gtk::glib::WeakRef<gtk::ListItem>, Rc<RowWidgets>)>> =
        const { RefCell::new(Vec::new()) };
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
        if let Some(notification) = &self.notification {
            clear_notification_row(notification);
        }
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
    ROW_WIDGETS.with(|entries| {
        let mut entries = entries.borrow_mut();
        entries.retain(|(weak, _)| weak.upgrade().is_some());
        if let Some((_, existing)) = entries
            .iter_mut()
            .find(|(weak, _)| weak.upgrade().is_some_and(|current| current == *item))
        {
            *existing = widgets;
        } else {
            entries.push((item.downgrade(), widgets));
        }
        // Keep a bounded fallback for unusual list-model churn before another cache access
        if entries.len() > MAX_TRACKED_ROW_WIDGETS {
            let excess = entries.len() - MAX_TRACKED_ROW_WIDGETS;
            entries.drain(..excess);
        }
    });
}

pub(super) fn get_row_widgets(item: &gtk::ListItem) -> Option<Rc<RowWidgets>> {
    ROW_WIDGETS.with(|entries| {
        let mut entries = entries.borrow_mut();
        entries.retain(|(weak, _)| weak.upgrade().is_some());
        entries
            .iter()
            .find(|(weak, _)| weak.upgrade().is_some_and(|current| current == *item))
            .map(|(_, widgets)| widgets.clone())
    })
}

#[cfg(test)]
#[path = "tests/widgets.rs"]
mod tests;
