use anyhow::Result;
use unixnotis_core::util;

use crate::cli::{Command, DndState};
use crate::main_log_follow::follow_debug_logs;
use crate::main_output::{print_inhibitors, print_notifications};

use super::client::ControlClient;
use super::output_gate::{allow_full_output, warn_full_requires_diagnostic};

pub(crate) async fn handle_command(client: &impl ControlClient, command: Command) -> Result<()> {
    // CLI forwards work to the daemon
    match command {
        Command::TogglePanel => {
            // Simple toggle keeps the daemon in control of its own visibility rules
            client.toggle_panel().await?;
        }
        Command::OpenPanel { debug } => {
            // Debug mode opens the panel and streams daemon logs for real-time triage
            if let Some(level) = debug {
                client.open_panel_debug(level.into()).await?;
                // Panel open should still succeed when journal follow is unavailable
                if let Err(err) = follow_debug_logs() {
                    eprintln!("debug log follow unavailable: {err}");
                }
            } else {
                client.open_panel().await?;
            }
        }
        Command::ClosePanel => {
            // Explicit close avoids accidental toggles when the panel is hidden
            client.close_panel().await?;
        }
        Command::Clear | Command::ClearAll => {
            // Clear keeps legacy behavior: remove active notifications and saved history
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
        Command::ListActive { full } => {
            let diagnostic_mode = util::diagnostic_mode();
            let allow_full = allow_full_output(full, diagnostic_mode);
            if warn_full_requires_diagnostic(full, diagnostic_mode) {
                // Fall back to the safe view
                eprintln!("--full requires UNIXNOTIS_DIAGNOSTIC=1; using redacted output");
            }
            let notifications = client.list_active().await?;
            print_notifications("active", &notifications, allow_full);
        }
        Command::ListHistory { full } => {
            let diagnostic_mode = util::diagnostic_mode();
            let allow_full = allow_full_output(full, diagnostic_mode);
            if warn_full_requires_diagnostic(full, diagnostic_mode) {
                eprintln!("--full requires UNIXNOTIS_DIAGNOSTIC=1; using redacted output");
            }
            let notifications = client.list_history().await?;
            print_notifications("history", &notifications, allow_full);
        }
        Command::Dnd { state } => match state {
            DndState::On => {
                // Explicit enable avoids ambiguous scripts
                client.set_dnd(true).await?;
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
            println!("{token}");
        }
        Command::Uninhibit { id } => {
            // Token removal is safe to repeat if a previous call already released it
            client.uninhibit(id).await?;
        }
        Command::ListInhibitors => {
            let inhibitors = client.list_inhibitors().await?;
            print_inhibitors(&inhibitors);
        }
        Command::CssCheck | Command::Preset { .. } => {}
    }

    Ok(())
}
