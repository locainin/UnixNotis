//! Popup action dispatch helpers
//!
//! Keeps GTK click handlers small and leaves queue fallback rules in one place

use std::sync::OnceLock;

use tokio::sync::mpsc::error::TrySendError;
use tokio::sync::mpsc::Sender;
use tracing::debug;

use crate::dbus::UiCommand;

pub(super) fn try_send_command(tx: &Sender<UiCommand>, command: UiCommand) {
    // Avoid blocking the GTK thread; fall back to async send if the queue is full
    match tx.try_send(command) {
        // Fast path for normal queue availability
        Ok(()) => {}
        Err(TrySendError::Full(command)) => {
            // Full queues are retried through the small fallback channel
            enqueue_fallback(tx, command);
        }
        Err(TrySendError::Closed(command)) => {
            // Closed means the popup can no longer reach the runtime worker
            debug!(?command, "command channel closed; dropping UI action");
        }
    }
}

fn enqueue_fallback(tx: &Sender<UiCommand>, command: UiCommand) {
    // Use a bounded fallback queue to keep user actions flowing without spawning
    // unbounded async tasks when the main command channel is saturated
    const FALLBACK_QUEUE_CAPACITY: usize = 32;
    static FALLBACK: OnceLock<async_channel::Sender<UiCommand>> = OnceLock::new();

    let fallback = FALLBACK.get_or_init(|| {
        let (fallback_tx, fallback_rx) = async_channel::bounded(FALLBACK_QUEUE_CAPACITY);
        let target = tx.clone();
        gtk::glib::MainContext::default().spawn_local(async move {
            while let Ok(cmd) = fallback_rx.recv().await {
                // Exit quietly once the runtime receiver disappears
                if target.send(cmd).await.is_err() {
                    break;
                }
            }
        });
        fallback_tx
    });

    if fallback.try_send(command).is_err() {
        debug!("popup command fallback queue full; dropping UI action");
    }
}
