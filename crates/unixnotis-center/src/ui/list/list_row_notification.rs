//! Notification row widget construction and updates.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::prelude::*;
use tokio::sync::mpsc;
use tracing::debug;
use unixnotis_core::{NotificationView, Urgency};

use crate::dbus::UiCommand;

use super::list_item::RowData;
use super::super::icons::IconResolver;
use super::super::try_send_command;

pub(super) struct NotificationRowWidgets {
    pub(super) icon: gtk::Image,
    pub(super) app_label: gtk::Label,
    pub(super) summary_label: gtk::Label,
    pub(super) body_label: gtk::Label,
    pub(super) actions_box: gtk::Box,
    pub(super) notify_id: Rc<Cell<u32>>,
    pub(super) action_cache: RefCell<Vec<(String, String)>>,
    pub(super) icon_sig: RefCell<Option<IconSignature>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct IconSignature {
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

pub(super) fn build_notification_row(
    command_tx: mpsc::Sender<UiCommand>,
) -> (gtk::Box, NotificationRowWidgets) {
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
    close_button.set_halign(gtk::Align::End);
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
        // Non-blocking enqueue avoids GTK stalls during D-Bus backpressure.
        try_send_command(&close_tx, UiCommand::Dismiss(id));
    });

    (
        root,
        NotificationRowWidgets {
            icon,
            app_label,
            summary_label,
            body_label,
            actions_box,
            notify_id,
            action_cache: RefCell::new(Vec::new()),
            icon_sig: RefCell::new(None),
        },
    )
}

pub(super) fn update_notification_row(
    row: &NotificationRowWidgets,
    root: &gtk::Box,
    data: &RowData,
    icon_resolver: &IconResolver,
    command_tx: &mpsc::Sender<UiCommand>,
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

    update_actions(
        &row.actions_box,
        &row.action_cache,
        command_tx,
        notification,
    );

    let next_sig = IconSignature::from(notification);
    let mut sig_guard = row.icon_sig.borrow_mut();
    if sig_guard.as_ref() != Some(&next_sig) {
        let scale = root.scale_factor();
        icon_resolver.apply_icon(&row.icon, notification, 22, scale);
        *sig_guard = Some(next_sig);
    }
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
    command_tx: &mpsc::Sender<UiCommand>,
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
            // Best-effort enqueue keeps action handling responsive.
            try_send_command(
                &tx,
                UiCommand::InvokeAction {
                    id,
                    action_key: action_key.clone(),
                },
            );
        });
        actions_box.append(&button);
    }
}
