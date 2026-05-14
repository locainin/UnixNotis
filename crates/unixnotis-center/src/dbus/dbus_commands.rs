use std::collections::VecDeque;

use tokio::sync::mpsc;
use tracing::warn;
use unixnotis_core::{ControlProxy, PanelDebugLevel};
use zbus::Result as ZbusResult;

use super::dbus_types::UiCommand;
use crate::debug;

// Cap offline queue length so a dead bus does not keep growing memory use
const MAX_OFFLINE_COMMANDS: usize = 128;

pub(crate) async fn handle_command(
    proxy: &ControlProxy<'_>,
    // The runtime still passes the UI sender here so the call shape stays uniform
    // Clear-all recovery no longer needs it because daemon invalidation handles reseed
    _sender: &async_channel::Sender<super::dbus_types::UiEvent>,
    command: UiCommand,
) -> ZbusResult<()> {
    match command {
        // Per-row actions still map straight to the daemon methods
        UiCommand::Dismiss(id) => proxy.dismiss(id).await,
        UiCommand::InvokeAction { id, action_key } => proxy.invoke_action(id, &action_key).await,
        // Daemon invalidation now drives refresh for every client, not just the caller
        // Keeping the caller path thin avoids reintroducing one-client-only fixes later
        UiCommand::ClearAll => proxy.clear_all().await,
        // State and visibility commands remain safe to replay after reconnect
        UiCommand::SetDnd(enabled) => proxy.set_dnd(enabled).await,
        UiCommand::ClosePanel => proxy.close_panel().await,
    }
}

pub(crate) fn stash_offline_commands(
    command_rx: &mut mpsc::Receiver<UiCommand>,
    offline: &mut VecDeque<UiCommand>,
) {
    let mut drained = 0usize;
    while let Ok(command) = command_rx.try_recv() {
        if enqueue_offline_command(offline, command) {
            drained += 1;
        }
    }
    if drained > 0 {
        debug::log(PanelDebugLevel::Info, || {
            format!(
                "buffered {drained} control command(s) while offline (queued={})",
                offline.len()
            )
        });
    }
}

fn enqueue_offline_command(offline: &mut VecDeque<UiCommand>, command: UiCommand) -> bool {
    match &command {
        // Close and clear are one-shot intents, so one buffered copy is enough
        UiCommand::ClearAll | UiCommand::ClosePanel => {
            if offline.iter().any(|queued| queued == &command) {
                // Duplicate one-shot replay adds no user value after reconnect
                return false;
            }
        }
        // DND should replay only the newest requested state after reconnect
        UiCommand::SetDnd(_) => {
            // Older states are stale once a newer DND request exists
            offline.retain(|queued| !matches!(queued, UiCommand::SetDnd(_)));
        }
        UiCommand::Dismiss(_) | UiCommand::InvokeAction { .. } => {}
    }

    if offline.len() >= MAX_OFFLINE_COMMANDS {
        // Drop the oldest buffered command first so recent intent wins
        offline.pop_front();
        warn!("dropping control command while interface is unavailable");
    }
    // Preserve command order so replay matches the original user actions
    offline.push_back(command);
    true
}

pub(crate) async fn flush_offline_commands(
    proxy: &ControlProxy<'_>,
    sender: &async_channel::Sender<super::dbus_types::UiEvent>,
    offline: &mut VecDeque<UiCommand>,
) {
    if offline.is_empty() {
        return;
    }
    // Replay after a fresh seed so stateful commands run against current daemon data
    debug::log(PanelDebugLevel::Info, || {
        format!("replaying {} buffered control command(s)", offline.len())
    });
    while let Some(command) = offline.pop_front() {
        if let Err(err) = handle_command(proxy, sender, command).await {
            warn!(?err, "buffered control command failed");
        }
    }
}

pub(crate) fn drop_stale_offline_commands(offline: &mut VecDeque<UiCommand>) {
    // Drop ID-based commands after reconnect to avoid acting on stale IDs.
    // Commands that do not depend on old notification ids are kept
    let before = offline.len();
    offline.retain(|command| {
        matches!(
            command,
            UiCommand::ClearAll | UiCommand::SetDnd(_) | UiCommand::ClosePanel
        )
    });
    let dropped = before.saturating_sub(offline.len());
    if dropped > 0 {
        debug::log(PanelDebugLevel::Info, || {
            format!("dropped {dropped} stale offline command(s) after reconnect")
        });
    }
}

#[cfg(test)]
#[path = "tests/commands.rs"]
mod tests;
