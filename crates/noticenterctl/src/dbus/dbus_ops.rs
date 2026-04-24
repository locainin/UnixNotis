//! D-Bus command execution for noticenterctl.

use std::future::Future;
use std::time::Duration;

use anyhow::{anyhow, Result};
use unixnotis_core::{util, ControlProxy};

use crate::cli_args::{Command, DndState};
use crate::main_log_follow::follow_debug_logs;
use crate::main_output::{print_inhibitors, print_notifications};

const CONTROL_CALL_TIMEOUT: Duration = Duration::from_secs(5);

pub(crate) async fn handle_command(proxy: &ControlProxy<'_>, command: Command) -> Result<()> {
    // CLI forwards work to the daemon
    match command {
        Command::TogglePanel => {
            // Simple toggle keeps the daemon in control of its own visibility rules.
            run_control_call(proxy.toggle_panel()).await?;
        }
        Command::OpenPanel { debug } => {
            // Debug mode opens the panel and streams daemon logs for real-time triage.
            if let Some(level) = debug {
                run_control_call(proxy.open_panel_debug(level.into())).await?;
                // Panel open should still succeed when journal follow is unavailable.
                if let Err(err) = follow_debug_logs() {
                    eprintln!("debug log follow unavailable: {err}");
                }
            } else {
                run_control_call(proxy.open_panel()).await?;
            }
        }
        Command::ClosePanel => {
            // Explicit close avoids accidental toggles when the panel is hidden.
            run_control_call(proxy.close_panel()).await?;
        }
        Command::Clear => {
            // Clear removes both active notifications and history entries.
            run_control_call(proxy.clear_all()).await?;
        }
        Command::Dismiss { id } => {
            // Dismiss targets a single notification by id.
            run_control_call(proxy.dismiss(id)).await?;
        }
        Command::ListActive { full } => {
            // Full output needs the debug gate
            let allow_full = full && util::diagnostic_mode();
            if full && !util::diagnostic_mode() {
                // Fall back to the safe view
                eprintln!("--full requires UNIXNOTIS_DIAGNOSTIC=1; using redacted output");
            }
            let notifications = run_control_call(proxy.list_active()).await?;
            // Shared output helper
            print_notifications("active", &notifications, allow_full);
        }
        Command::ListHistory { full } => {
            // Same gate for history
            let allow_full = full && util::diagnostic_mode();
            if full && !util::diagnostic_mode() {
                eprintln!("--full requires UNIXNOTIS_DIAGNOSTIC=1; using redacted output");
            }
            let notifications = run_control_call(proxy.list_history()).await?;
            print_notifications("history", &notifications, allow_full);
        }
        Command::Dnd { state } => match state {
            DndState::On => {
                // Explicit enable avoids ambiguous scripts.
                run_control_call(proxy.set_dnd(true)).await?;
            }
            DndState::Off => {
                // Explicit disable avoids ambiguous scripts.
                run_control_call(proxy.set_dnd(false)).await?;
            }
            DndState::Toggle => {
                // Toggle must happen atomically in the daemon to avoid read-modify-write races.
                run_control_call(proxy.toggle_dnd()).await?;
            }
        },
        Command::Inhibit { reason, scope } => {
            // Print the token only
            let token = run_control_call(proxy.inhibit(&reason, scope.as_scope())).await?;
            println!("{token}");
        }
        Command::Uninhibit { id } => {
            // Token removal is safe to repeat if a previous call already released it.
            run_control_call(proxy.uninhibit(id)).await?;
        }
        Command::ListInhibitors => {
            let inhibitors = run_control_call(proxy.list_inhibitors()).await?;
            // Shared output helper
            print_inhibitors(&inhibitors);
        }
        Command::CssCheck => {
            // CSS validation is handled before D-Bus connection setup.
        }
        Command::Preset { .. } => {
            // Preset commands are handled before D-Bus connection setup.
        }
    }

    Ok(())
}

async fn run_control_call<T>(call: impl Future<Output = zbus::Result<T>>) -> Result<T> {
    match tokio::time::timeout(CONTROL_CALL_TIMEOUT, call).await {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(err)) => Err(err.into()),
        Err(_) => Err(anyhow!(
            "timed out waiting for unixnotis daemon response after {}s",
            CONTROL_CALL_TIMEOUT.as_secs()
        )),
    }
}
