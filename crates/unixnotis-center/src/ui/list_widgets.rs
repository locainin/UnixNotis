//! Row widgets and rendering logic for the notification list.
//!
//! Keeps GTK widget creation and updates isolated from list state.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use async_channel::Sender;
use gtk::prelude::*;
use gtk::{self, Align};
use tokio::sync::mpsc::UnboundedSender;
use tracing::debug;
use unixnotis_core::{NotificationView, Urgency};

use crate::dbus::{UiCommand, UiEvent};

use super::super::icons::IconResolver;
use super::super::list_item::{RowData, RowItem, RowKind};

/// GTK wrapper widgets for each row type.
pub(super) struct RowWidgets {
    kind: RowKind,
    root: gtk::Box,
    group: Option<GroupRowWidgets>,
    notification: Option<NotificationRowWidgets>,
    ghost: Option<GhostRowWidgets>,
    handler: RefCell<Option<(RowItem, gtk::glib::SignalHandlerId)>>,
    command_tx: UnboundedSender<UiCommand>,
}

struct GroupRowWidgets {
    icon: gtk::Image,
    title: gtk::Label,
    count: gtk::Label,
    chevron: gtk::Image,
    group_key: Rc<RefCell<Rc<str>>>,
}

struct NotificationRowWidgets {
    icon: gtk::Image,
    app_label: gtk::Label,
    summary_label: gtk::Label,
    body_label: gtk::Label,
    actions_box: gtk::Box,
    notify_id: Rc<Cell<u32>>,
    action_cache: RefCell<Vec<(String, String)>>,
    icon_sig: RefCell<Option<IconSignature>>,
}

struct GhostRowWidgets {
    depth: RefCell<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct IconSignature {
    image_path: String,
    icon_name: String,
    app_name: String,
    has_image_data: bool,
    image_len: usize,
    image_width: i32,
    image_height: i32,
}

impl IconSignature {
    fn from(notification: &NotificationView) -> Self {
        Self {
            image_path: notification.image.image_path.clone(),
            icon_name: notification.image.icon_name.clone(),
            app_name: notification.app_name.clone(),
            has_image_data: notification.image.has_image_data,
            image_len: notification.image.image_data.data.len(),
            image_width: notification.image.image_data.width,
            image_height: notification.image.image_data.height,
        }
    }
}

impl RowWidgets {
    pub(super) fn new(
        kind: RowKind,
        command_tx: UnboundedSender<UiCommand>,
        event_tx: Sender<UiEvent>,
    ) -> Self {
        match kind {
            RowKind::GroupHeader => Self::new_group(command_tx, event_tx),
            RowKind::Notification => Self::new_notification(command_tx),
            RowKind::Ghost => Self::new_ghost(command_tx),
        }
    }

    fn new_group(command_tx: UnboundedSender<UiCommand>, event_tx: Sender<UiEvent>) -> Self {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 6);
        root.add_css_class("unixnotis-group");
        root.add_css_class("unixnotis-group-row");

        let button = gtk::Button::new();
        button.add_css_class("unixnotis-group-header");
        button.set_has_frame(false);
        button.set_focusable(false);

        let header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let icon = gtk::Image::new();
        icon.set_pixel_size(18);
        icon.add_css_class("unixnotis-group-icon");

        let title = gtk::Label::new(None);
        title.set_xalign(0.0);
        title.add_css_class("unixnotis-group-title");

        let count = gtk::Label::new(Some("0"));
        count.set_xalign(0.5);
        count.add_css_class("unixnotis-group-count");

        let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 1);
        spacer.set_hexpand(true);

        let chevron = gtk::Image::from_icon_name("pan-down-symbolic");
        chevron.add_css_class("unixnotis-group-chevron");

        header.append(&icon);
        header.append(&title);
        header.append(&spacer);
        header.append(&count);
        header.append(&chevron);
        button.set_child(Some(&header));
        root.append(&button);

        let group_key: Rc<RefCell<Rc<str>>> = Rc::new(RefCell::new(Rc::from("")));
        let event_tx_clone = event_tx.clone();
        let group_key_clone = group_key.clone();
        button.connect_clicked(move |_| {
            let group = group_key_clone.borrow().clone();
            if group.is_empty() {
                return;
            }
            let _ = event_tx_clone.try_send(UiEvent::GroupToggled(group.to_string()));
        });

        Self {
            kind: RowKind::GroupHeader,
            root,
            group: Some(GroupRowWidgets {
                icon,
                title,
                count,
                chevron,
                group_key,
            }),
            notification: None,
            ghost: None,
            handler: RefCell::new(None),
            command_tx,
        }
    }

    fn new_notification(command_tx: UnboundedSender<UiCommand>) -> Self {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 6);
        root.add_css_class("unixnotis-panel-card");

        let header = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let icon = gtk::Image::new();
        icon.set_pixel_size(22);
        icon.add_css_class("unixnotis-panel-icon");

        let app_label = gtk::Label::new(None);
        app_label.set_xalign(0.0);
        app_label.add_css_class("unixnotis-panel-app");

        let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 1);
        spacer.set_hexpand(true);

        let close_button = gtk::Button::from_icon_name("window-close-symbolic");
        close_button.set_halign(Align::End);
        close_button.add_css_class("unixnotis-panel-close");

        header.append(&icon);
        header.append(&app_label);
        header.append(&spacer);
        header.append(&close_button);

        let summary_label = gtk::Label::new(None);
        summary_label.set_xalign(0.0);
        summary_label.set_wrap(true);
        summary_label.add_css_class("unixnotis-panel-summary");

        let body_label = gtk::Label::new(None);
        body_label.set_xalign(0.0);
        body_label.set_wrap(true);
        body_label.add_css_class("unixnotis-panel-body");

        let actions_box = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        actions_box.add_css_class("unixnotis-notification-actions");

        root.append(&header);
        root.append(&summary_label);
        root.append(&body_label);
        root.append(&actions_box);

        let notify_id = Rc::new(Cell::new(0));
        let close_tx = command_tx.clone();
        let notify_id_clone = notify_id.clone();
        close_button.connect_clicked(move |_| {
            let id = notify_id_clone.get();
            if id == 0 {
                return;
            }
            debug!(id, "dismiss clicked");
            let _ = close_tx.send(UiCommand::Dismiss(id));
        });

        Self {
            kind: RowKind::Notification,
            root,
            group: None,
            notification: Some(NotificationRowWidgets {
                icon,
                app_label,
                summary_label,
                body_label,
                actions_box,
                notify_id,
                action_cache: RefCell::new(Vec::new()),
                icon_sig: RefCell::new(None),
            }),
            ghost: None,
            handler: RefCell::new(None),
            command_tx,
        }
    }

    fn new_ghost(command_tx: UnboundedSender<UiCommand>) -> Self {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.add_css_class("unixnotis-panel-card");
        root.add_css_class("unixnotis-stack-ghost");
        root.set_visible(true);

        Self {
            kind: RowKind::Ghost,
            root,
            group: None,
            notification: None,
            ghost: Some(GhostRowWidgets {
                depth: RefCell::new(0),
            }),
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
    command_tx: UnboundedSender<UiCommand>,
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
        // Store on the list item to avoid a root <-> widget reference cycle.
        list_item.set_data("unixnotis-row-widgets", widgets);
    }
}

pub(super) fn get_row_widgets(list_item: &gtk::ListItem) -> Option<Rc<RowWidgets>> {
    unsafe {
        list_item
            .data::<Rc<RowWidgets>>("unixnotis-row-widgets")
            .map(|ptr| ptr.as_ref().clone())
    }
}

fn update_group_row(
    group: &GroupRowWidgets,
    root: &gtk::Box,
    data: &RowData,
    icon_resolver: &IconResolver,
) {
    group.title.set_text(data.group_key.as_ref());
    group.count.set_text(&format!("{}", data.count));
    let chevron_name = if data.expanded {
        "pan-up-symbolic"
    } else {
        "pan-down-symbolic"
    };
    group.chevron.set_icon_name(Some(chevron_name));
    if data.expanded {
        root.remove_css_class("collapsed");
    } else {
        root.add_css_class("collapsed");
    }

    *group.group_key.borrow_mut() = data.group_key.clone();

    if let Some(notification) = data.notification.as_ref() {
        let scale = root.scale_factor();
        icon_resolver.apply_icon(&group.icon, notification.as_ref(), 18, scale);
    } else {
        group.icon.set_visible(false);
    }
}

fn update_notification_row(
    row: &NotificationRowWidgets,
    root: &gtk::Box,
    data: &RowData,
    icon_resolver: &IconResolver,
    command_tx: &UnboundedSender<UiCommand>,
) {
    let Some(notification) = data.notification.as_ref() else {
        return;
    };
    let notification = notification.as_ref();

    if notification.urgency == Urgency::Critical as u8 {
        root.add_css_class("critical");
    } else {
        root.remove_css_class("critical");
    }
    if data.is_active {
        root.add_css_class("active");
    } else {
        root.remove_css_class("active");
    }
    if data.stacked {
        root.add_css_class("stacked");
    } else {
        root.remove_css_class("stacked");
    }

    row.app_label.set_text(&notification.app_name);
    row.summary_label.set_text(&notification.summary);
    update_body_label(&row.body_label, &notification.body);
    row.notify_id.set(notification.id);

    update_actions(&row.actions_box, &row.action_cache, command_tx, notification);

    let next_sig = IconSignature::from(notification);
    let mut sig_guard = row.icon_sig.borrow_mut();
    if sig_guard.as_ref() != Some(&next_sig) {
        let scale = root.scale_factor();
        icon_resolver.apply_icon(&row.icon, notification, 22, scale);
        *sig_guard = Some(next_sig);
    }
}

fn update_ghost_row(ghost: &GhostRowWidgets, root: &gtk::Box, data: &RowData) {
    let mut depth = ghost.depth.borrow_mut();
    if *depth == data.ghost_depth {
        return;
    }
    if *depth > 0 {
        root.remove_css_class(&format!("unixnotis-stack-ghost-{}", *depth));
    }
    if data.ghost_depth > 0 {
        root.add_css_class(&format!("unixnotis-stack-ghost-{}", data.ghost_depth));
    }
    *depth = data.ghost_depth;
}

fn update_body_label(label: &gtk::Label, body: &str) {
    if body.is_empty() {
        label.set_text("");
        label.set_visible(false);
        return;
    }
    label.set_visible(true);
    label.set_markup(body);
}

fn update_actions(
    actions_box: &gtk::Box,
    cache: &RefCell<Vec<(String, String)>>,
    command_tx: &UnboundedSender<UiCommand>,
    notification: &NotificationView,
) {
    {
        let cached = cache.borrow();
        if cached.len() == notification.actions.len()
            && cached
                .iter()
                .zip(notification.actions.iter())
                .all(|((key, label), action)| key == &action.key && label == &action.label)
        {
            return;
        }
    }

    {
        let mut cached = cache.borrow_mut();
        cached.clear();
        cached.reserve(notification.actions.len());
        for action in &notification.actions {
            cached.push((action.key.clone(), action.label.clone()));
        }
    }

    // Refresh action buttons only when the action list changes.
    while let Some(child) = actions_box.first_child() {
        actions_box.remove(&child);
    }
    if notification.actions.is_empty() {
        return;
    }

    for action in &notification.actions {
        let button = gtk::Button::with_label(&action.label);
        button.add_css_class("unixnotis-panel-action");
        button.add_css_class("unixnotis-notification-action");
        let action_key = action.key.clone();
        let tx = command_tx.clone();
        let id = notification.id;
        button.connect_clicked(move |_| {
            debug!(id, action = %action_key, "action invoked");
            let _ = tx.send(UiCommand::InvokeAction {
                id,
                action_key: action_key.clone(),
            });
        });
        actions_box.append(&button);
    }
}
