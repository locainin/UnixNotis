use anyhow::Result;
use unixnotis_core::util;

use crate::cli::{Command, DevCommand, DndState};
use crate::output::{
    print_inhibitors, print_notification_diagnostics, print_notifications, require_diagnostic_mode,
    write_stdout,
};

use super::client::ControlClient;

pub async fn handle_command(client: &impl ControlClient, command: Command) -> Result<()> {
    // Keep library-level dispatch safe even when a caller bypasses the CLI runner
    command.validate()?;
    // CLI forwards work to the daemon
    match command {
        Command::TogglePanel => {
            // Simple toggle keeps the daemon in control of its own visibility rules
            client.toggle_panel().await?;
        }
        Command::OpenPanel => {
            // Normal panel opening never changes daemon diagnostic rendering
            client.open_panel().await?;
        }
        Command::ClosePanel => {
            // Explicit close avoids accidental toggles when the panel is hidden
            client.close_panel().await?;
        }
        Command::Clear => {
            // Clear removes active notifications and saved history through one daemon call
            client.clear_all().await?;
        }
        Command::ClearActive => {
            client.clear_active().await?;
        }
        Command::ClearHistory => {
            client.clear_history().await?;
        }
        Command::Dismiss { id } => {
            // Dismiss targets a single notification by id
            client.dismiss(id).await?;
        }
        Command::ListActive => {
            // Normal lists always use the compact bounded formatter
            let notifications = client.list_active().await?;
            print_notifications("active", &notifications, false)?;
        }
        Command::ListHistory => {
            // History follows the same safe default as the active list
            let notifications = client.list_history().await?;
            print_notifications("history", &notifications, false)?;
        }
        Command::Dnd {
            state,
            for_duration,
            until,
        } => match state {
            DndState::On => {
                let expires_at = match (for_duration, until) {
                    (Some(duration), None) => Some(duration.deadline()?),
                    (None, Some(clock)) => Some(clock.deadline()?),
                    (None, None) => None,
                    // Clap rejects this pair, but keep dispatch defensive for direct tests
                    (Some(_), Some(_)) => {
                        return Err(anyhow::anyhow!("--for and --until cannot be used together"));
                    }
                };
                if let Some(expires_at) = expires_at {
                    client.set_dnd_until(expires_at).await?;
                } else {
                    // Explicit enable without timing means indefinite DND
                    client.set_dnd(true).await?;
                }
            }
            DndState::Off => {
                // Explicit disable avoids ambiguous scripts
                client.set_dnd(false).await?;
            }
            DndState::Toggle => {
                // Toggle must happen atomically in the daemon to avoid read-modify-write races
                client.toggle_dnd().await?;
            }
        },
        Command::Inhibit { reason, scope } => {
            let token = client.inhibit(&reason, scope.as_scope()).await?;
            write_stdout(&format!("{token}\n"))?;
        }
        Command::Uninhibit { id } => {
            // Token removal is safe to repeat if a previous call already released it
            client.uninhibit(id).await?;
        }
        Command::ListInhibitors => {
            let inhibitors = client.list_inhibitors().await?;
            print_inhibitors(&inhibitors)?;
        }
        Command::Dev {
            command: DevCommand::Logs,
        } => {
            anyhow::bail!("internal routing error: dev logs reached daemon dispatcher");
        }
        Command::Dev { command } => {
            // Developer D-Bus commands still use the normal client and timeout boundary
            handle_dev_command(client, command).await?;
        }
        Command::CssCheck { .. }
        | Command::Doctor { .. }
        | Command::Preset { .. }
        | Command::Theme { .. } => {
            anyhow::bail!("internal routing error: local command reached daemon dispatcher");
        }
    }

    Ok(())
}

async fn handle_dev_command(client: &impl ControlClient, command: DevCommand) -> Result<()> {
    // Read diagnostic mode once so each command has one consistent security decision
    handle_dev_command_with_diagnostic_mode(client, command, util::diagnostic_mode()).await
}

pub(super) async fn handle_dev_command_with_diagnostic_mode(
    client: &impl ControlClient,
    command: DevCommand,
    diagnostic_mode: bool,
) -> Result<()> {
    match command {
        DevCommand::OpenPanel { level } => {
            // Debug rendering is independent from journal log following
            client.open_panel_debug(level.into()).await?;
        }
        DevCommand::RefreshApplications => {
            client.refresh_applications().await?;
        }
        DevCommand::ExplainNotification { id } => {
            let mut diagnostics = client.notification_diagnostics(id).await?;
            let view = diagnostics
                .pop()
                .ok_or_else(|| anyhow::anyhow!("notification {id} is not active"))?;
            // Structured formatting keeps client-controlled text terminal-safe and bounded
            print_notification_diagnostics(&view)?;
        }
        DevCommand::DumpActive => {
            // Reject before fetching data so a denied dump has no daemon side effects
            require_diagnostic_mode(diagnostic_mode)?;
            let notifications = client.list_active().await?;
            print_notifications("active", &notifications, true)?;
        }
        DevCommand::DumpHistory => {
            require_diagnostic_mode(diagnostic_mode)?;
            let notifications = client.list_history().await?;
            print_notifications("history", &notifications, true)?;
        }
        DevCommand::Logs => {
            anyhow::bail!("internal routing error: dev logs reached daemon dispatcher");
        }
    }

    Ok(())
}
