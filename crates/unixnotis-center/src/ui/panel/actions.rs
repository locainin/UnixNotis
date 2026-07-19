//! Panel action signal wiring

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use gtk::prelude::*;
use tracing::debug;

use super::super::try_send_command;
use super::input::ClickCooldown;
use super::timing::CONTROL_CLICK_GUARD_MS;
use super::PanelWidgets;
use crate::control::UiCommand;

pub(in crate::ui) fn connect_clear_button(
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

pub(in crate::ui) fn connect_dnd_toggle(
    panel: &PanelWidgets,
    dnd_guard: Rc<Cell<bool>>,
    command_tx: tokio::sync::mpsc::Sender<UiCommand>,
) {
    connect_dnd_button(&panel.dnd_toggle, dnd_guard, command_tx);
}

fn connect_dnd_button(
    button: &gtk::ToggleButton,
    dnd_guard: Rc<Cell<bool>>,
    command_tx: tokio::sync::mpsc::Sender<UiCommand>,
) {
    button.connect_toggled(move |button| {
        if dnd_guard.get() {
            // Daemon-driven state sync should not echo another DND command
            return;
        }

        let requested = button.is_active();
        // Keep the durable daemon state visible until the command commits successfully
        dnd_guard.set(true);
        button.set_active(!requested);
        dnd_guard.set(false);
        debug!(enabled = requested, "dnd toggled");
        try_send_command(&command_tx, UiCommand::SetDnd(requested));
    });
}

pub(in crate::ui) fn connect_close_button(
    panel: &PanelWidgets,
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

#[cfg(test)]
#[path = "tests/actions.rs"]
mod tests;
