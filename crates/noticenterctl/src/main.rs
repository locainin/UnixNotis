#![allow(
    clippy::blanket_clippy_restriction_lints,
    clippy::nursery,
    clippy::pedantic,
    clippy::restriction,
    reason = "workspace clippy runs use these groups as review signals, not as zero-tolerance policy gates"
)]

//! Command-line control surface for the UnixNotis D-Bus interface

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
    // Parse CLI arguments first so local-only commands can stop here
    let args = Args::parse();

    if args.command.is_local_only() {
        // Local-only commands skip D-Bus setup on purpose
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

    // Connect to the session bus before proxying control commands
    let connection = Connection::session()
        .await
        .context("connect to session bus")?;
    let proxy = ControlProxy::new(&connection)
        .await
        .context("connect to unixnotis control interface")?;

    // Hand command execution to the D-Bus operation layer
    dbus_ops::handle_command(&proxy, args.command).await?;
    Ok(())
}
