//! Group header row widget construction and updates
//!
//! Group rows own the header controls used to expand and collapse grouped items

use std::cell::RefCell;
use std::rc::Rc;

use async_channel::{Sender, TrySendError};
use gtk::prelude::*;
use tracing::debug;
use unixnotis_core::{css::hooks, util};

use crate::dbus::UiEvent;

use super::super::super::icons::IconResolver;
use super::super::list_item::RowData;

pub(in crate::ui::list) struct GroupRowWidgets {
    pub(super) icon: gtk::Image,
    pub(super) title: gtk::Label,
    pub(super) count: gtk::Label,
    pub(super) chevron: gtk::Image,
    pub(super) group_key: Rc<RefCell<Rc<str>>>,
}

pub(in crate::ui::list) fn build_group_row(
    event_tx: Sender<UiEvent>,
) -> (gtk::Box, GroupRowWidgets) {
    // Root container groups the header and any future expansion widgets
    let root = gtk::Box::new(gtk::Orientation::Vertical, 6);
    root.add_css_class(hooks::group_row::ROOT);
    root.add_css_class(hooks::group_row::CONTAINER);
    root.add_css_class(hooks::group_row::EXPANDED);

    let button = gtk::Button::new();
    button.add_css_class(hooks::group_row::HEADER);
    button.set_has_frame(false);
    button.set_focusable(true);
    button.set_tooltip_text(Some("Toggle group"));

    let header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let icon = gtk::Image::new();
    icon.set_pixel_size(18);
    icon.add_css_class(hooks::group_row::ICON);

    let title = gtk::Label::new(None);
    title.set_xalign(0.0);
    title.add_css_class(hooks::group_row::TITLE);

    let count = gtk::Label::new(Some("0"));
    count.set_xalign(0.5);
    count.add_css_class(hooks::group_row::COUNT);

    let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 1);
    spacer.set_hexpand(true);

    let chevron = gtk::Image::from_icon_name("pan-down-symbolic");
    chevron.add_css_class(hooks::group_row::CHEVRON);

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
        // UI actions are high-priority
        // If the bounded queue is full, enqueue asynchronously
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

pub(in crate::ui::list) fn update_group_row(
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
    // Display the original app label while the normalized key drives grouping behavior
    // Fall back to the group key if no sample notification is available
    set_label_text_if_changed(&group.title, display_name);
    let next_count = data.count.to_string();
    set_label_text_if_changed(&group.count, &next_count);
    let chevron_name = if data.expanded {
        "pan-up-symbolic"
    } else {
        "pan-down-symbolic"
    };
    set_icon_name_if_changed(&group.chevron, chevron_name);
    set_class_state(root, hooks::group_row::COLLAPSED, !data.expanded);
    set_class_state(root, hooks::group_row::EXPANDED, data.expanded);

    *group.group_key.borrow_mut() = data.group_key.clone();

    if let Some(notification) = data.notification.as_ref() {
        let scale = root.scale_factor();
        icon_resolver.apply_icon(&group.icon, notification.as_ref(), 18, scale);
        set_class_state(root, hooks::group_row::HAS_ICON, true);
        set_class_state(root, hooks::group_row::NO_ICON, false);
    } else {
        set_widget_visible_if_changed(&group.icon, false);
        set_class_state(root, hooks::group_row::NO_ICON, true);
        set_class_state(root, hooks::group_row::HAS_ICON, false);
    }
}

fn set_label_text_if_changed(label: &gtk::Label, text: &str) {
    // Repeated model refreshes often land on the same text
    // Skip the setter when the rendered value already matches
    if label.text().as_str() != text {
        label.set_text(text);
    }
}

fn set_icon_name_if_changed(image: &gtk::Image, icon_name: &str) {
    // Chevron updates are common while grouping changes settle
    // Avoid reassigning the same symbolic icon over and over
    if image.icon_name().as_deref() != Some(icon_name) {
        image.set_icon_name(Some(icon_name));
    }
}

fn set_widget_visible_if_changed<W: IsA<gtk::Widget>>(widget: &W, visible: bool) {
    // Visibility flips trigger GTK work even when the value is unchanged
    // Guard the setter so empty groups do not keep re-hiding the same widget
    if widget.is_visible() != visible {
        widget.set_visible(visible);
    }
}

fn set_class_state(widget: &gtk::Box, class_name: &str, enabled: bool) {
    // CSS state stays cheap when no-op toggles are skipped
    if enabled {
        if !widget.has_css_class(class_name) {
            widget.add_css_class(class_name);
        }
    } else if widget.has_css_class(class_name) {
        widget.remove_css_class(class_name);
    }
}
