//! Command-line control surface for the UnixNotis D-Bus interface.

mod cli_args;
#[path = "dbus/dbus_ops.rs"]
mod dbus_ops;
#[path = "main/main_css_check.rs"]
mod main_css_check;
#[path = "main/main_log_follow.rs"]
mod main_log_follow;
#[path = "main/main_output.rs"]
mod main_output;
mod preset;

use anyhow::{Context, Result};
use clap::Parser;
use cli_args::Args;
use unixnotis_core::ControlProxy;
use zbus::Connection;

#[tokio::main]
async fn main() -> Result<()> {
    // Parse CLI arguments first so early-return commands can skip D-Bus setup.
    let args = Args::parse();

    if args.command.is_local_only() {
        // Local-only commands intentionally run without a D-Bus connection.
        match args.command {
            cli_args::Command::CssCheck => {
                main_css_check::run_css_check()?;
            }
            cli_args::Command::Preset { command } => {
                preset::run_preset(command).context("preset command failed")?;
            }
            _ => {}
        }
        return Ok(());
    }

    // Connect to the session bus and proxy the control interface.
    let connection = Connection::session()
        .await
        .context("connect to session bus")?;
    let proxy = ControlProxy::new(&connection)
        .await
        .context("connect to unixnotis control interface")?;

    // Delegate command execution to keep main focused on setup/teardown.
    dbus_ops::handle_command(&proxy, args.command).await?;
    Ok(())
}
