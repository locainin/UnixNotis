//! Dedicated Tokio runtime construction and queue wiring

use std::thread;

use tokio::sync::{mpsc, watch};
use tracing::warn;

use super::connection::run_dbus_loop;
use super::{PopupRuntime, UI_COMMAND_QUEUE_CAPACITY};
use crate::dbus::{UiCommand, UiEvent};

pub(super) fn start_runtime(sender: async_channel::Sender<UiEvent>) -> PopupRuntime {
    let (command_tx, command_rx) = mpsc::channel(UI_COMMAND_QUEUE_CAPACITY);
    let (gtk_ready_tx, gtk_ready_rx) = watch::channel(false);
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    spawn_runtime_thread(sender, command_rx, gtk_ready_rx, shutdown_rx);
    PopupRuntime {
        command_tx,
        gtk_ready_tx,
        shutdown_tx,
    }
}

fn spawn_runtime_thread(
    sender: async_channel::Sender<UiEvent>,
    command_rx: mpsc::Receiver<UiCommand>,
    gtk_ready_rx: watch::Receiver<bool>,
    shutdown_rx: watch::Receiver<bool>,
) {
    thread::spawn(move || {
        // The GTK main thread never blocks on bus calls or retry delays
        let Some(runtime) = build_runtime() else {
            return;
        };
        runtime.block_on(run_dbus_loop(sender, command_rx, gtk_ready_rx, shutdown_rx));
    });
}

pub(super) fn build_runtime() -> Option<tokio::runtime::Runtime> {
    tokio::runtime::Builder::new_multi_thread()
        // Two workers keep signal delivery moving while one bounded call is pending
        .worker_threads(2)
        .enable_all()
        .build()
        .map_err(|error| {
            warn!(?error, "failed to initialize popup Tokio runtime");
            error
        })
        .ok()
}
