//! Non-blocking command delivery from GTK handlers to the D-Bus runtime

use tokio::sync::mpsc;
use tokio::sync::mpsc::error::TrySendError;

use crate::control::UiCommand;

pub fn try_send_command(command_tx: &mpsc::Sender<UiCommand>, command: UiCommand) {
    // Non-blocking send keeps GTK handlers responsive under D-Bus stalls
    match command_tx.try_send(command) {
        Ok(()) => {}
        Err(TrySendError::Full(command)) => {
            // Backpressure is retried asynchronously to avoid dropping user actions
            let command_tx = command_tx.clone();
            glib::MainContext::default().spawn_local(async move {
                if let Err(err) = command_tx.send(command).await {
                    tracing::warn!(?err, "failed to enqueue ui command after backpressure");
                }
            });
        }
        Err(TrySendError::Closed(command)) => {
            tracing::warn!(?command, "ui command dropped because channel closed");
        }
    }
}

#[cfg(test)]
#[path = "tests/command.rs"]
mod tests;
