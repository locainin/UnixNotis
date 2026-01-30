//! Command handling and offline queue management for center D-Bus actions.

use std::collections::VecDeque;

use tokio::sync::mpsc;
use tracing::warn;
use unixnotis_core::{ControlProxy, PanelDebugLevel};
use zbus::Result as ZbusResult;

use crate::debug;

use super::dbus_seed::seed_state;
use super::dbus_types::UiCommand;

// Cap offline queue length to avoid unbounded memory use when the bus is unavailable.
const MAX_OFFLINE_COMMANDS: usize = 128;

pub(crate) async fn handle_command(
    proxy: &ControlProxy<'_>,
    sender: &async_channel::Sender<super::dbus_types::UiEvent>,
    command: UiCommand,
) -> ZbusResult<()> {
    match command {
        UiCommand::Dismiss(id) => proxy.dismiss(id).await,
        UiCommand::InvokeAction { id, action_key } => proxy.invoke_action(id, &action_key).await,
        UiCommand::ClearAll => {
            proxy.clear_all().await?;
            if let Err(err) = seed_state(proxy, sender).await {
                warn!(
                    state_error = ?err.state_error,
                    active_error = ?err.active_error,
                    history_error = ?err.history_error,
                    "failed to refresh center state after clear"
                );
            }
            Ok(())
        }
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
        if offline.len() >= MAX_OFFLINE_COMMANDS {
            offline.pop_front();
            warn!("dropping control command while interface is unavailable");
        }
        offline.push_back(command);
        drained += 1;
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

pub(crate) async fn flush_offline_commands(
    proxy: &ControlProxy<'_>,
    sender: &async_channel::Sender<super::dbus_types::UiEvent>,
    offline: &mut VecDeque<UiCommand>,
) {
    if offline.is_empty() {
        return;
    }
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
mod tests {
    use super::*;

    #[test]
    fn drop_stale_offline_commands_retains_safe_actions() {
        let mut offline = VecDeque::new();
        offline.push_back(UiCommand::Dismiss(10));
        offline.push_back(UiCommand::InvokeAction {
            id: 11,
            action_key: "open".to_string(),
        });
        offline.push_back(UiCommand::SetDnd(true));
        offline.push_back(UiCommand::ClearAll);
        offline.push_back(UiCommand::ClosePanel);

        drop_stale_offline_commands(&mut offline);

        assert_eq!(offline.len(), 3);
        assert!(offline
            .iter()
            .any(|cmd| matches!(cmd, UiCommand::SetDnd(true))));
        assert!(offline.iter().any(|cmd| matches!(cmd, UiCommand::ClearAll)));
        assert!(offline
            .iter()
            .any(|cmd| matches!(cmd, UiCommand::ClosePanel)));
    }
}
