//! Popup D-Bus command helpers

use tokio::sync::mpsc;
use tracing::warn;
use unixnotis_core::{timed_dbus_call, ControlProxy};
use zbus::Result as ZbusResult;

use super::types::UiCommand;

pub async fn handle_command(proxy: &ControlProxy<'_>, command: UiCommand) -> ZbusResult<()> {
    match command {
        UiCommand::Dismiss(notification) => {
            timed_dbus_call(proxy.dismiss_generation(notification.id, notification.generation))
                .await
        }
        UiCommand::InvokeAction {
            notification,
            action_key,
        } => {
            timed_dbus_call(proxy.invoke_action_generation(
                notification.id,
                notification.generation,
                &action_key,
            ))
            .await
        }
        UiCommand::Reply {
            id,
            generation,
            text,
            outcome,
        } => {
            let result = timed_dbus_call(proxy.reply_notification(id, generation, &text)).await;
            let reply_result = result.as_ref().map_err(ToString::to_string).copied();
            let _ = outcome.send(reply_result);
            result
        }
        UiCommand::Materialized(notification) => {
            timed_dbus_call(proxy.mark_popup_materialized(notification.id, notification.generation))
                .await
        }
        UiCommand::Visible(notification) => {
            timed_dbus_call(proxy.mark_popup_visible(notification.id, notification.generation))
                .await
        }
        UiCommand::Shutdown(_) => Ok(()),
    }
}

pub fn drain_offline_commands(
    command_rx: &mut mpsc::Receiver<UiCommand>,
) -> Option<std::sync::mpsc::SyncSender<()>> {
    while let Ok(command) = command_rx.try_recv() {
        match command {
            UiCommand::Shutdown(acknowledgement) => return Some(acknowledgement),
            UiCommand::Reply { outcome, .. } => {
                let _ = outcome.send(Err("notification service is unavailable".to_string()));
            }
            UiCommand::Dismiss(_)
            | UiCommand::InvokeAction { .. }
            | UiCommand::Materialized(_)
            | UiCommand::Visible(_) => {}
        }
        // Popups only reflect live state, so stale button actions are dropped while offline
        warn!("dropping control command while interface is unavailable");
    }
    None
}

#[cfg(test)]
#[path = "tests/commands.rs"]
mod tests;
