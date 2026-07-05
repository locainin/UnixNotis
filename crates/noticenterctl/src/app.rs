//! Application runner for noticenterctl

use anyhow::{Context, Result};
use clap::Parser;
use unixnotis_core::ControlProxy;
use zbus::Connection;

use crate::cli::{Args, Command};

pub(crate) async fn run() -> Result<()> {
    // Parse CLI arguments before any daemon work starts
    let args = Args::parse();

    if args.command.is_local_only() {
        // Local commands must work even when the daemon is not running
        match args.command {
            Command::CssCheck => {
                crate::css_check::run_css_check()?;
            }
            Command::Preset { command } => {
                crate::preset::run_preset(command).context("preset command failed")?;
            }
            _ => {}
        }
        return Ok(());
    }

    // Control commands need the session bus and the daemon proxy
    let connection = Connection::session()
        .await
        .context("connect to session bus")?;
    let proxy = ControlProxy::new(&connection)
        .await
        .context("connect to unixnotis control interface")?;

    crate::dbus::handle_command(&proxy, args.command).await
}
