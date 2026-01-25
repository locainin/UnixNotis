//! Row widgets and rendering logic for the notification list.
//!
//! Keeps GTK widget creation and updates isolated from list state.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::OnceLock;

use async_channel::Sender;
use gtk::prelude::*;
use gtk::{self};
use tokio::sync::mpsc;
use tracing::debug;

use crate::dbus::{UiCommand, UiEvent};

use super::super::icons::IconResolver;
use super::list_item::{RowData, RowItem, RowKind};
use super::list_row_ghost::{build_ghost_row, update_ghost_row, GhostRowWidgets};
use super::list_row_group::{build_group_row, update_group_row, GroupRowWidgets};
use super::list_row_notification::{
    build_notification_row, update_notification_row, NotificationRowWidgets,
};

/// GTK wrapper widgets for each row type.
pub(super) struct RowWidgets {
    kind: RowKind,
    root: gtk::Box,
    group: Option<GroupRowWidgets>,
    notification: Option<NotificationRowWidgets>,
    ghost: Option<GhostRowWidgets>,
    handler: RefCell<Option<(RowItem, gtk::glib::SignalHandlerId)>>,
    command_tx: mpsc::Sender<UiCommand>,
}

fn row_widgets_quark() -> gtk::glib::Quark {
    static QUARK: OnceLock<gtk::glib::Quark> = OnceLock::new();
    *QUARK.get_or_init(|| gtk::glib::Quark::from_str("unixnotis-row-widgets"))
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
            RowKind::Ghost => Self::new_ghost(command_tx),
        }
    }

    fn new_group(command_tx: mpsc::Sender<UiCommand>, event_tx: Sender<UiEvent>) -> Self {
        let (root, group) = build_group_row(event_tx);

        Self {
            kind: RowKind::GroupHeader,
            root,
            group: Some(group),
            notification: None,
            ghost: None,
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
            ghost: None,
            handler: RefCell::new(None),
            command_tx,
        }
    }

    fn new_ghost(command_tx: mpsc::Sender<UiCommand>) -> Self {
        let (root, ghost) = build_ghost_row();

        Self {
            kind: RowKind::Ghost,
            root,
            group: None,
            notification: None,
            ghost: Some(ghost),
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
                    update_notification_row(
                        notification,
                        &self.root,
                        data,
                        icon_resolver,
                        &self.command_tx,
                    );
                }
            }
            RowKind::Ghost => {
                if let Some(ghost) = &self.ghost {
                    update_ghost_row(ghost, &self.root, data);
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
    list_item: &gtk::ListItem,
    kind: RowKind,
    command_tx: mpsc::Sender<UiCommand>,
    event_tx: Sender<UiEvent>,
) -> Rc<RowWidgets> {
    if let Some(existing) = get_row_widgets(list_item) {
        if existing.kind == kind {
            return existing.clone();
        }
    }

    let widgets = Rc::new(RowWidgets::new(kind, command_tx, event_tx));
    list_item.set_child(Some(&widgets.root));
    set_row_widgets(list_item, widgets.clone());
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
    let icon_resolver = icon_resolver.clone();
    let handler = item.connect_local("updated", false, move |_| {
        let data = item_clone.data();
        widgets_clone.refresh(&data, &icon_resolver);
        None
    });
    *widgets.handler.borrow_mut() = Some((item.clone(), handler));
}

pub(super) fn set_row_widgets(list_item: &gtk::ListItem, widgets: Rc<RowWidgets>) {
    unsafe {
        // SAFETY: gtk::ListItem stays on the GTK main thread and never crosses threads.
        // RowWidgets uses Rc and is only accessed from list factory callbacks on the
        // main thread. Data is replaced in ensure_row_widgets when the row kind changes
        // and otherwise kept to let GTK reuse the row widgets across scroll events.
        list_item.set_qdata(row_widgets_quark(), widgets);
    }
}

pub(super) fn get_row_widgets(list_item: &gtk::ListItem) -> Option<Rc<RowWidgets>> {
    unsafe {
        list_item
            .qdata::<Rc<RowWidgets>>(row_widgets_quark())
            .map(|ptr| ptr.as_ref().clone())
    }
}
