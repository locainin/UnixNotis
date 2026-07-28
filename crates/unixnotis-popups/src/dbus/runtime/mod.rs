//! Popup D-Bus runtime public surface

mod bootstrap;
mod connection;
mod delivery;
mod generation;
mod readiness;

use tokio::sync::{mpsc, watch};

use super::types::{UiCommand, UiEvent};

// A bounded queue prevents a stalled D-Bus connection from growing memory without limit
pub(super) const UI_COMMAND_QUEUE_CAPACITY: usize = 64;

pub struct PopupRuntime {
    command_tx: mpsc::Sender<UiCommand>,
    gtk_ready_tx: watch::Sender<bool>,
}

impl PopupRuntime {
    pub fn command_sender(&self) -> mpsc::Sender<UiCommand> {
        self.command_tx.clone()
    }

    pub fn mark_gtk_ready(&self) {
        // Readiness is published only after the GTK state owns its complete widget tree
        let _ = self.gtk_ready_tx.send(true);
    }
}

pub fn start_dbus_runtime(sender: async_channel::Sender<UiEvent>) -> PopupRuntime {
    bootstrap::start_runtime(sender)
}

#[cfg(test)]
mod tests;
