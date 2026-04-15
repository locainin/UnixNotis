//! Popup action dispatch helpers
//!
//! Keeps GTK click handlers small and leaves queue fallback rules in one place

use tokio::sync::mpsc::error::TrySendError;
use tokio::sync::mpsc::Sender;
use tracing::debug;

use crate::dbus::UiCommand;

pub(super) fn try_send_command(tx: &Sender<UiCommand>, command: UiCommand) {
    // GTK click handlers must stay non-blocking even when the runtime queue is saturated
    match tx.try_send(command) {
        // Fast path for normal queue availability
        Ok(()) => {}
        Err(TrySendError::Full(command)) => {
            // One deferred retry is enough here because popup actions are low-volume and user-driven
            enqueue_async_send(tx.clone(), command);
        }
        Err(TrySendError::Closed(command)) => {
            // Closed means the popup can no longer reach the runtime worker
            debug!(?command, "command channel closed; dropping UI action");
        }
    }
}

fn enqueue_async_send(tx: Sender<UiCommand>, command: UiCommand) {
    // Reuse the same channel instead of creating a second fallback queue with its own backlog
    gtk::glib::MainContext::default().spawn_local(async move {
        // If the runtime is gone, the popup can no longer deliver actions and should just stop
        if tx.send(command).await.is_err() {
            debug!("popup command channel closed during deferred send");
        }
    });
}
