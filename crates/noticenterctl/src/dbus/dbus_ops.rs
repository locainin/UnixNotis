//! D-Bus command execution for noticenterctl.

use anyhow::Result;
use unixnotis_core::{util, ControlProxy};

use crate::cli_args::{Command, DndState};
use crate::main_log_follow::follow_debug_logs;
use crate::main_output::print_notifications;

pub(crate) async fn handle_command(proxy: &ControlProxy<'_>, command: Command) -> Result<()> {
    match command {
        Command::TogglePanel => {
            // Simple toggle keeps the daemon in control of its own visibility rules.
            proxy.toggle_panel().await?;
        }
        Command::OpenPanel { debug } => {
            // Debug mode opens the panel and streams daemon logs for real-time triage.
            if let Some(level) = debug {
                proxy.open_panel_debug(level.into()).await?;
                follow_debug_logs()?;
            } else {
                proxy.open_panel().await?;
            }
        }
        Command::ClosePanel => {
            // Explicit close avoids accidental toggles when the panel is hidden.
            proxy.close_panel().await?;
        }
        Command::Clear => {
            // Clear removes active notifications but preserves history.
            proxy.clear_all().await?;
        }
        Command::Dismiss { id } => {
            // Dismiss targets a single notification by id.
            proxy.dismiss(id).await?;
        }
        Command::ListActive { full } => {
            // Full output is only allowed in diagnostic mode to avoid leaking content.
            let allow_full = full && util::diagnostic_mode();
            if full && !util::diagnostic_mode() {
                eprintln!("--full requires UNIXNOTIS_DIAGNOSTIC=1; using redacted output");
            }
            let notifications = proxy.list_active().await?;
            print_notifications("active", &notifications, allow_full);
        }
        Command::ListHistory { full } => {
            // History output follows the same diagnostic gating as active list.
            let allow_full = full && util::diagnostic_mode();
            if full && !util::diagnostic_mode() {
                eprintln!("--full requires UNIXNOTIS_DIAGNOSTIC=1; using redacted output");
            }
            let notifications = proxy.list_history().await?;
            print_notifications("history", &notifications, allow_full);
        }
        Command::Dnd { state } => match state {
            DndState::On => {
                // Explicit enable avoids ambiguous scripts.
                proxy.set_dnd(true).await?;
            }
            DndState::Off => {
                // Explicit disable avoids ambiguous scripts.
                proxy.set_dnd(false).await?;
            }
            DndState::Toggle => {
                // Read current state from the daemon to avoid local drift.
                let current = proxy.get_state().await?;
                proxy.set_dnd(!current.dnd_enabled).await?;
            }
        },
        Command::Inhibit { reason, scope } => {
            // Print the token so callers can release the inhibitor later.
            let token = proxy.inhibit(&reason, scope.as_scope()).await?;
            println!("{token}");
        }
        Command::Uninhibit { id } => {
            // Token removal is safe to repeat if a previous call already released it.
            proxy.uninhibit(id).await?;
        }
        Command::ListInhibitors => {
            let inhibitors = proxy.list_inhibitors().await?;
            println!("inhibitors: {}", inhibitors.len());
            for (id, reason, scope, owner) in inhibitors {
                println!("- #{id} scope={scope} owner={owner} reason={reason}");
            }
        }
        Command::CssCheck => {
            // CSS validation is handled before D-Bus connection setup.
        }
    }

    Ok(())
}
