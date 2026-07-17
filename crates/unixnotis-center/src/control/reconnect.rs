//! Session-bus reconnection and control-generation lifecycle

use std::collections::VecDeque;
use std::time::Duration;

use tokio::sync::mpsc;
use tracing::info;
use unixnotis_core::ControlProxy;
use zbus::Connection;

use super::backoff::{
    Backoff, RetryLog, BACKOFF_BASE_MS, BACKOFF_MAX_MS, RETRY_WARN_INTERVAL_SECS,
};
use super::commands::stash_offline_commands;
use super::model::{UiCommand, UiEvent};
use super::subscriptions::run_control_generation;

#[cfg(test)]
#[path = "tests/reconnect.rs"]
mod reconnect_tests;

pub(super) async fn run_control_loop(
    sender: async_channel::Sender<UiEvent>,
    mut command_rx: mpsc::Receiver<UiCommand>,
) {
    // Buffer UI actions during reconnect to avoid losing user intent.
    let mut offline_commands: VecDeque<UiCommand> = VecDeque::new();
    let mut connect_backoff = Backoff::new(BACKOFF_BASE_MS, BACKOFF_MAX_MS);
    let mut subscribe_backoff = Backoff::new(BACKOFF_BASE_MS, BACKOFF_MAX_MS);
    let mut connect_log = RetryLog::new(Duration::from_secs(RETRY_WARN_INTERVAL_SECS));
    let mut subscribe_log = RetryLog::new(Duration::from_secs(RETRY_WARN_INTERVAL_SECS));

    loop {
        // A disconnected zbus socket is terminal, so each loop owns a fresh bus generation
        let connection = match Connection::session().await {
            Ok(connection) => connection,
            Err(err) => {
                connect_log.warn_or_debug(&err, "failed to connect to session bus; retrying");
                stash_offline_commands(&mut command_rx, &mut offline_commands);
                tokio::time::sleep(connect_backoff.next_sleep()).await;
                continue;
            }
        };
        connect_backoff.reset();
        connect_log.reset();

        let proxy = match ControlProxy::new(&connection).await {
            Ok(proxy) => proxy,
            Err(err) => {
                connect_log.warn_or_debug(&err, "control interface unavailable, retrying");
                stash_offline_commands(&mut command_rx, &mut offline_commands);
                tokio::time::sleep(connect_backoff.next_sleep()).await;
                continue;
            }
        };
        info!("connected to unixnotis control interface");

        let generation = run_control_generation(
            &proxy,
            &sender,
            &mut command_rx,
            &mut offline_commands,
            &mut subscribe_backoff,
            &mut subscribe_log,
        )
        .await;
        if !generation.requires_reconnect_cleanup() {
            continue;
        }
        stash_offline_commands(&mut command_rx, &mut offline_commands);
        tokio::time::sleep(subscribe_backoff.next_sleep()).await;
    }
}
