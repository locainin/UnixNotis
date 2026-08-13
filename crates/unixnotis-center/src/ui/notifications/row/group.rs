//! Group header row widget construction and updates
//!
//! Group rows own the header controls used to expand and collapse grouped items

use std::cell::RefCell;
use std::rc::Rc;

use async_channel::{Sender, TrySendError};
use gtk::pango;
use gtk::prelude::*;
use tracing::debug;
use unixnotis_core::{css::hooks, util};
use unixnotis_ui::presentation::{NotificationPresentation, TrustLevel};

use crate::control::UiEvent;

use super::super::super::icons::IconResolver;
use super::super::item::RowData;

const GROUP_AVATAR_SIZE: i32 = 26;
const GROUP_ICON_SIZE: i32 = 18;
pub(in crate::ui::notifications) struct GroupRowWidgets {
    pub(super) button: gtk::Button,
    pub(super) avatar: gtk::Box,
    pub(super) icon: gtk::Image,
    pub(super) title: gtk::Label,
    pub(super) secondary: gtk::Label,
    pub(super) trust_chip: gtk::Label,
    pub(super) count: gtk::Label,
    pub(super) chevron: gtk::Image,
    pub(super) group_key: Rc<RefCell<Rc<str>>>,
}

pub(in crate::ui::notifications) fn build_group_row(
    event_tx: Sender<UiEvent>,
) -> (gtk::Box, GroupRowWidgets) {
    // Root container groups the header and any future expansion widgets
    let root = gtk::Box::new(gtk::Orientation::Vertical, 6);
    root.add_css_class(hooks::group_row::ROOT);
    root.add_css_class(hooks::group_row::CONTAINER);
    root.add_css_class(hooks::group_row::EXPANDED);
    root.set_hexpand(true);
    root.set_halign(gtk::Align::Fill);
    root.set_vexpand(false);
    root.set_margin_bottom(super::layout::NOTIFICATION_LIST_ROW_GAP);

    let button = gtk::Button::new();
    button.add_css_class(hooks::group_row::HEADER);
    button.set_has_frame(false);
    button.set_focusable(true);
    button.set_tooltip_text(Some("Toggle group"));
    button.set_hexpand(true);
    button.set_halign(gtk::Align::Fill);

    let header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    header.set_hexpand(true);
    header.set_halign(gtk::Align::Fill);
    let avatar = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    avatar.set_halign(gtk::Align::Center);
    avatar.set_valign(gtk::Align::Center);
    avatar.set_size_request(GROUP_AVATAR_SIZE, GROUP_AVATAR_SIZE);
    avatar.add_css_class("unixnotis-group-avatar");
    let icon = gtk::Image::new();
    icon.set_pixel_size(GROUP_ICON_SIZE);
    icon.add_css_class(hooks::group_row::ICON);
    avatar.append(&icon);

    let identity = gtk::Box::new(gtk::Orientation::Vertical, 1);
    identity.set_hexpand(true);
    let identity_top = gtk::Box::new(gtk::Orientation::Horizontal, 6);

    let title = gtk::Label::new(None);
    title.set_xalign(0.0);
    title.set_ellipsize(pango::EllipsizeMode::End);
    title.set_single_line_mode(true);
    title.add_css_class(hooks::group_row::TITLE);

    let trust_chip = gtk::Label::new(None);
    trust_chip.set_single_line_mode(true);
    trust_chip.add_css_class("unixnotis-group-trust-chip");
    trust_chip.set_visible(false);

    let secondary = gtk::Label::new(None);
    secondary.set_xalign(0.0);
    secondary.set_ellipsize(pango::EllipsizeMode::End);
    secondary.set_single_line_mode(true);
    secondary.add_css_class("unixnotis-group-secondary");
    secondary.set_visible(false);

    let count = gtk::Label::new(Some("0"));
    count.set_xalign(0.5);
    count.add_css_class(hooks::group_row::COUNT);

    let chevron = gtk::Image::from_icon_name("pan-down-symbolic");
    chevron.set_pixel_size(14);
    chevron.add_css_class(hooks::group_row::CHEVRON);

    identity_top.append(&title);
    identity_top.append(&trust_chip);
    identity.append(&identity_top);
    identity.append(&secondary);
    header.append(&avatar);
    header.append(&identity);
    header.append(&count);
    header.append(&chevron);
    button.set_child(Some(&header));
    root.append(&button);

    let group_key: Rc<RefCell<Rc<str>>> = Rc::new(RefCell::new(Rc::from("")));
    let event_tx_clone = event_tx;
    let group_key_clone = group_key.clone();
    button.connect_clicked(move |button| {
        if !button.is_sensitive() {
            // Programmatic signal emission must respect the same singleton guard
            return;
        }
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
            button,
            avatar,
            icon,
            title,
            secondary,
            trust_chip,
            count,
            chevron,
            group_key,
        },
    )
}

pub(in crate::ui::notifications) fn update_group_row(
    group: &GroupRowWidgets,
    root: &gtk::Box,
    data: &RowData,
    icon_resolver: &IconResolver,
) {
    let presentation = data
        .notification
        .as_ref()
        .map(|notification| NotificationPresentation::from_view(notification));
    let display_name = presentation
        .as_ref()
        .map(|view| view.identity.primary_label.as_str())
        .filter(|name| !name.is_empty())
        .unwrap_or(data.group_key.as_ref());
    // Display application presentation while the daemon identity key drives grouping behavior
    // Fall back to the group key if no sample notification is available
    set_label_text_if_changed(&group.title, display_name);
    let secondary = presentation
        .as_ref()
        .and_then(|view| view.identity.secondary_claim.as_deref())
        .unwrap_or_default();
    set_label_text_if_changed(&group.secondary, secondary);
    set_widget_visible_if_changed(&group.secondary, !secondary.is_empty());
    let trust_label = presentation
        .as_ref()
        .and_then(|view| view.trust.short_label.as_deref())
        .unwrap_or_default();
    set_label_text_if_changed(&group.trust_chip, trust_label);
    set_widget_visible_if_changed(&group.trust_chip, !trust_label.is_empty());
    let next_count = data.count.to_string();
    set_label_text_if_changed(&group.count, &next_count);
    let has_multiple = data.count > 1;
    set_widget_visible_if_changed(&group.count, has_multiple);
    set_widget_visible_if_changed(&group.chevron, has_multiple);
    group.button.set_focusable(has_multiple);
    group.button.set_sensitive(has_multiple);
    group
        .button
        .set_tooltip_text(has_multiple.then_some("Toggle notification group"));
    let accessible_label = group_accessible_label(
        display_name,
        trust_label,
        secondary,
        data.count,
        data.expanded,
    );
    group
        .button
        .update_property(&[gtk::accessible::Property::Label(&accessible_label)]);
    let chevron_name = if data.expanded {
        "pan-up-symbolic"
    } else {
        "pan-down-symbolic"
    };
    if has_multiple {
        set_icon_name_if_changed(&group.chevron, chevron_name);
    }
    set_class_state(root, hooks::group_row::COLLAPSED, !data.expanded);
    set_class_state(root, hooks::group_row::EXPANDED, data.expanded);

    *group.group_key.borrow_mut() = data.group_key.clone();

    if let Some(notification) = data.notification.as_ref() {
        let Some(presentation) = presentation else {
            root.queue_resize();
            return;
        };
        set_widget_visible_if_changed(&group.avatar, true);
        if presentation.trust.details_label.is_none() {
            group.title.set_tooltip_text(None);
        } else if let Some(details) = presentation.trust.details_label.as_deref() {
            group.title.set_tooltip_text(Some(details));
        }
        set_class_state(
            root,
            "unixnotis-attribution-warning",
            presentation.trust.level == TrustLevel::Conflict,
        );
        for (level, class_name) in [
            (TrustLevel::Verified, "verified"),
            (TrustLevel::Unresolved, "unresolved"),
            (TrustLevel::Conflict, "conflict"),
            (TrustLevel::Relay, "relay"),
        ] {
            set_class_state(root, class_name, presentation.trust.level == level);
        }
        set_class_state(
            root,
            "recognized",
            matches!(
                presentation.trust.level,
                TrustLevel::SystemAssociated
                    | TrustLevel::PortalAssociated
                    | TrustLevel::UserAssociated
            ),
        );
        let scale = root.scale_factor();
        icon_resolver.apply_identity_badge(
            &group.icon,
            notification.as_ref(),
            presentation.identity.badge,
            presentation.trust.level,
            GROUP_ICON_SIZE,
            scale,
        );
        set_class_state(root, hooks::group_row::HAS_ICON, true);
        set_class_state(root, hooks::group_row::NO_ICON, false);
    } else {
        clear_group_identity(group, icon_resolver);
        clear_group_trust_state(group, root);
        set_class_state(root, hooks::group_row::NO_ICON, true);
        set_class_state(root, hooks::group_row::HAS_ICON, false);
    }
    // Group identity changes can alter the natural row height when rows are recycled
    root.queue_resize();
}

pub(in crate::ui::notifications) fn clear_group_identity(
    group: &GroupRowWidgets,
    icon_resolver: &IconResolver,
) {
    // Recycled group rows must revoke ownership of pending async icon work
    // Hiding the widget alone is insufficient because a late decode shows it again
    icon_resolver.clear_identity_badge(&group.icon);
    set_widget_visible_if_changed(&group.avatar, false);
}

fn clear_group_trust_state(group: &GroupRowWidgets, root: &gtk::Box) {
    // An empty model sample carries no trust evidence from the previous recycled row
    group.title.set_tooltip_text(None);
    set_class_state(root, "unixnotis-attribution-warning", false);
    for class_name in ["verified", "recognized", "unresolved", "conflict", "relay"] {
        set_class_state(root, class_name, false);
    }
}

fn group_accessible_label(
    display_name: &str,
    trust_label: &str,
    secondary: &str,
    count: u32,
    expanded: bool,
) -> String {
    let mut parts = vec![display_name.trim().to_string()];
    if !trust_label.trim().is_empty() {
        parts.push(trust_label.trim().to_string());
    }
    if !secondary.trim().is_empty() {
        parts.push(secondary.trim().to_string());
    }
    parts.push(if count == 1 {
        "1 notification".to_string()
    } else {
        format!("{count} notifications")
    });
    if count > 1 {
        parts.push(if expanded { "Expanded" } else { "Collapsed" }.to_string());
    }
    parts.join(". ")
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
    if widget.get_visible() != visible {
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

#[cfg(test)]
#[path = "tests/group.rs"]
mod tests;
