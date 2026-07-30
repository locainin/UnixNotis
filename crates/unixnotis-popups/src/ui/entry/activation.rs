//! Whole-card default action activation and interactive-child isolation

use gtk::prelude::*;
use unixnotis_core::NotificationKey;

use super::commands::try_send_command;
use super::presentation::PopupEntryViewModel;
use crate::dbus::UiCommand;

pub(super) const INTERACTIVE_CLASS: &str = "unixnotis-popup-interactive";

pub(super) fn mark_interactive<W: IsA<gtk::Widget>>(widget: &W) {
    // One explicit marker protects current controls and future composite widgets
    widget.add_css_class(INTERACTIVE_CLASS);
}

pub(super) fn connect_default_action(
    root: &gtk::Box,
    notification: NotificationKey,
    view: &PopupEntryViewModel,
    command_tx: &tokio::sync::mpsc::Sender<UiCommand>,
) {
    let Some(action_key) = view.default_action_key.clone() else {
        return;
    };

    // A blank-label default action still needs a discoverable keyboard target
    root.set_focusable(true);
    root.set_accessible_role(gtk::AccessibleRole::Button);
    root.update_property(&[gtk::accessible::Property::Label("Open notification")]);
    root.add_css_class("unixnotis-popup-default-action");

    let gesture = gtk::GestureClick::new();
    gesture.set_button(1);
    let root_weak = root.downgrade();
    let click_tx = command_tx.clone();
    let click_key = action_key.clone();
    gesture.connect_released(move |_, _, x, y| {
        let Some(root) = root_weak.upgrade() else {
            return;
        };
        dispatch_default_action(
            root.upcast_ref(),
            root.pick(x, y, gtk::PickFlags::DEFAULT),
            notification,
            &click_key,
            &click_tx,
        );
    });
    root.add_controller(gesture);

    let key_controller = gtk::EventControllerKey::new();
    let root_weak = root.downgrade();
    let key_tx = command_tx.clone();
    key_controller.connect_key_pressed(move |_, key, _, _| {
        let Some(root) = root_weak.upgrade() else {
            return gtk::glib::Propagation::Proceed;
        };
        handle_default_action_key(root.has_focus(), key, notification, &action_key, &key_tx)
    });
    root.add_controller(key_controller);
}

fn handle_default_action_key(
    root_has_focus: bool,
    key: gtk::gdk::Key,
    notification: NotificationKey,
    action_key: &str,
    command_tx: &tokio::sync::mpsc::Sender<UiCommand>,
) -> gtk::glib::Propagation {
    if !root_has_focus || !is_default_activation_key(key) {
        return gtk::glib::Propagation::Proceed;
    }
    invoke_default_action(notification, action_key, command_tx);
    gtk::glib::Propagation::Stop
}

fn dispatch_default_action(
    root: &gtk::Widget,
    picked: Option<gtk::Widget>,
    notification: NotificationKey,
    action_key: &str,
    command_tx: &tokio::sync::mpsc::Sender<UiCommand>,
) {
    if picked_widget_blocks_default_action(root, picked) {
        return;
    }
    invoke_default_action(notification, action_key, command_tx);
}

fn invoke_default_action(
    notification: NotificationKey,
    action_key: &str,
    command_tx: &tokio::sync::mpsc::Sender<UiCommand>,
) {
    // Presentation policy has already removed default actions from weak identities
    try_send_command(
        command_tx,
        UiCommand::InvokeAction {
            notification,
            action_key: action_key.to_string(),
            confirmed: false,
        },
    );
}

fn picked_widget_blocks_default_action(
    root: &gtk::Widget,
    mut picked: Option<gtk::Widget>,
) -> bool {
    while let Some(current) = picked {
        if current == *root {
            return false;
        }
        // Focusability is a safe fallback for controls not yet carrying the marker
        if current.has_css_class(INTERACTIVE_CLASS) || current.is_focusable() {
            return true;
        }
        picked = current.parent();
    }
    false
}

const fn is_default_activation_key(key: gtk::gdk::Key) -> bool {
    matches!(
        key,
        gtk::gdk::Key::Return | gtk::gdk::Key::KP_Enter | gtk::gdk::Key::space
    )
}

#[cfg(test)]
#[path = "tests/activation.rs"]
mod tests;
