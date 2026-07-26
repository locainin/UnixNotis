//! Popup D-Bus command helpers

use tokio::sync::mpsc;
use tracing::warn;
use unixnotis_core::{timed_dbus_call, ControlProxy};
use zbus::Result as ZbusResult;

use super::types::UiCommand;

pub async fn handle_command(proxy: &ControlProxy<'_>, command: UiCommand) -> ZbusResult<()> {
    match command {
        UiCommand::Dismiss(id) => timed_dbus_call(proxy.dismiss(id)).await,
        UiCommand::InvokeAction { id, action_key } => {
            timed_dbus_call(proxy.invoke_action(id, &action_key)).await
        }
        UiCommand::Shutdown(_) => Ok(()),
    }
}

pub fn drain_offline_commands(
    command_rx: &mut mpsc::Receiver<UiCommand>,
) -> Option<std::sync::mpsc::SyncSender<()>> {
    while let Ok(command) = command_rx.try_recv() {
        if let UiCommand::Shutdown(acknowledgement) = command {
            return Some(acknowledgement);
        }
        // Popups only reflect live state, so stale button actions are dropped while offline
        warn!("dropping control command while interface is unavailable");
    }
    None
}

#[cfg(test)]
#[path = "tests/commands.rs"]
mod tests;
