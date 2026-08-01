//! Whole-card default action activation and interactive-child isolation

use gtk::prelude::*;
use unixnotis_core::NotificationKey;
use unixnotis_ui::presentation::default_activation::{
    connect_default_activation, mark_interactive as shared_mark_interactive, DefaultActionTarget,
};

use super::commands::try_send_command;
use super::presentation::PopupEntryViewModel;
use crate::dbus::UiCommand;

pub(super) fn mark_interactive<W: IsA<gtk::Widget>>(widget: &W) {
    shared_mark_interactive(widget);
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
    let click_tx = command_tx.clone();
    let binding = connect_default_activation(root, move |notification, action_key| {
        try_send_command(
            &click_tx,
            UiCommand::InvokeAction {
                notification,
                action_key,
                confirmed: false,
            },
        );
    });
    binding.set_target(Some(DefaultActionTarget {
        notification,
        action_key,
    }));
}

#[cfg(test)]
#[path = "tests/activation.rs"]
mod tests;
