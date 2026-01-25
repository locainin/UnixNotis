//! Group header row widget construction and updates.

use std::cell::RefCell;
use std::rc::Rc;

use async_channel::{Sender, TrySendError};
use gtk::prelude::*;
use tracing::debug;
use unixnotis_core::util;

use crate::dbus::UiEvent;

use super::super::icons::IconResolver;
use super::list_item::RowData;

pub(super) struct GroupRowWidgets {
    pub(super) icon: gtk::Image,
    pub(super) title: gtk::Label,
    pub(super) count: gtk::Label,
    pub(super) chevron: gtk::Image,
    pub(super) group_key: Rc<RefCell<Rc<str>>>,
}

pub(super) fn build_group_row(event_tx: Sender<UiEvent>) -> (gtk::Box, GroupRowWidgets) {
    // Root container groups the header and any future expansion widgets.
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
        // UI actions are high-priority; if the bounded queue is full, enqueue asynchronously.
        match event_tx_clone.try_send(UiEvent::GroupToggled(group.to_string())) {
            Ok(()) => {}
            Err(TrySendError::Full(event)) => {
                let event_tx = event_tx_clone.clone();
                gtk::glib::MainContext::default().spawn_local(async move {
                    let _ = event_tx.send(event).await;
                });
            }
            Err(TrySendError::Closed(_)) => {
                let snippet = util::log_snippet(&group);
                debug!(
                    group = %snippet,
                    "group toggle dropped because event channel closed (likely shutdown)"
                );
            }
        }
    });

    (
        root,
        GroupRowWidgets {
            icon,
            title,
            count,
            chevron,
            group_key,
        },
    )
}

pub(super) fn update_group_row(
    group: &GroupRowWidgets,
    root: &gtk::Box,
    data: &RowData,
    icon_resolver: &IconResolver,
) {
    let display_name = data
        .notification
        .as_ref()
        .map(|notification| notification.app_name.trim())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| data.group_key.as_ref());
    // Display the original app label while the normalized key drives grouping behavior.
    // Fall back to the group key if no sample notification is available.
    group.title.set_text(display_name);
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
