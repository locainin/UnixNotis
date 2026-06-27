//! Panel action signal wiring

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use gtk::prelude::*;
use tracing::debug;

use super::super::input_guard::ClickCooldown;
use super::super::panel;
use super::super::try_send_command;
use super::timing::CONTROL_CLICK_GUARD_MS;
use crate::dbus::UiCommand;

pub(super) fn connect_clear_button(
    button: &gtk::Button,
    command_tx: tokio::sync::mpsc::Sender<UiCommand>,
) {
    let clear_gate = ClickCooldown::new(Duration::from_millis(CONTROL_CLICK_GUARD_MS));
    button.connect_clicked(move |_| {
        if !clear_gate.try_start() {
            return;
        }

        debug!("clear all clicked");
        // Non-blocking send avoids UI stalls on D-Bus backpressure
        try_send_command(&command_tx, UiCommand::ClearAll);
    });
}

pub(super) fn connect_dnd_toggle(
    panel: &panel::PanelWidgets,
    dnd_guard: Rc<Cell<bool>>,
    command_tx: tokio::sync::mpsc::Sender<UiCommand>,
) {
    panel.dnd_toggle.connect_toggled(move |button| {
        if dnd_guard.get() {
            // Daemon-driven state sync should not echo another DND command
            return;
        }

        debug!(enabled = button.is_active(), "dnd toggled");
        try_send_command(&command_tx, UiCommand::SetDnd(button.is_active()));
    });
}

pub(super) fn connect_close_button(
    panel: &panel::PanelWidgets,
    command_tx: tokio::sync::mpsc::Sender<UiCommand>,
) {
    let close_gate = ClickCooldown::new(Duration::from_millis(CONTROL_CLICK_GUARD_MS));
    panel.close_button.connect_clicked(move |_| {
        if !close_gate.try_start() {
            return;
        }

        debug!("close panel clicked");
        try_send_command(&command_tx, UiCommand::ClosePanel);
    });
}
