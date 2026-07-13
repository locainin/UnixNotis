//! Popup D-Bus command helpers

use tokio::sync::mpsc;
use tracing::warn;
use unixnotis_core::ControlProxy;
use zbus::Result as ZbusResult;

use super::dbus_types::UiCommand;

pub async fn handle_command(proxy: &ControlProxy<'_>, command: UiCommand) -> ZbusResult<()> {
    match command {
        UiCommand::Dismiss(id) => proxy.dismiss(id).await,
        UiCommand::InvokeAction { id, action_key } => proxy.invoke_action(id, &action_key).await,
    }
}

pub fn drain_offline_commands(command_rx: &mut mpsc::Receiver<UiCommand>) {
    while command_rx.try_recv().is_ok() {
        // Popups only reflect live state, so stale button actions are dropped while offline
        warn!("dropping control command while interface is unavailable");
    }
}

#[cfg(test)]
#[path = "tests/commands.rs"]
mod tests;
