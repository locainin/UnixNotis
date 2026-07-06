//! Application runner for noticenterctl

use anyhow::{Context, Result};
use clap::Parser;
use unixnotis_core::ControlProxy;
use zbus::Connection;

use crate::cli::{Args, Command};

pub(crate) async fn run() -> Result<()> {
    // Parse CLI arguments before any daemon work starts
    let args = Args::parse();
    let command = args.command;

    if command.is_local_only() {
        // Local commands must work even when the daemon is not running
        handle_local_command(command, crate::css_check::run, crate::preset::run_preset)?;
        return Ok(());
    }

    // Control commands need the session bus and the daemon proxy
    let connection = Connection::session()
        .await
        .context("connect to session bus")?;
    let proxy = ControlProxy::new(&connection)
        .await
        .context("connect to unixnotis control interface")?;

    crate::dbus::handle_command(&proxy, command).await
}

fn handle_local_command(
    command: Command,
    mut run_css: impl FnMut() -> Result<()>,
    mut run_preset: impl FnMut(crate::cli::PresetCommand) -> Result<()>,
) -> Result<()> {
    match command {
        Command::CssCheck => run_css(),
        Command::Preset { command } => run_preset(command).context("preset command failed"),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use anyhow::Result;

    use crate::cli::{Command, PresetCommand};

    use super::handle_local_command;

    #[test]
    fn handle_local_command_runs_css_check_branch() {
        let css_called = Cell::new(false);

        handle_local_command(
            Command::CssCheck,
            || {
                css_called.set(true);
                Ok(())
            },
            |_| -> Result<()> { panic!("preset runner should not be called for css check") },
        )
        .expect("css check should dispatch");

        assert!(css_called.get());
    }

    #[test]
    fn handle_local_command_runs_preset_branch_with_command_payload() {
        let preset_called = Cell::new(false);

        handle_local_command(
            Command::Preset {
                command: PresetCommand::Inspect {
                    input: "theme.unixnotis".to_string(),
                },
            },
            || -> Result<()> { panic!("css runner should not be called for preset command") },
            |command| {
                let PresetCommand::Inspect { input } = command else {
                    panic!("expected inspect preset command");
                };
                assert_eq!(input, "theme.unixnotis");
                preset_called.set(true);
                Ok(())
            },
        )
        .expect("preset should dispatch");

        assert!(preset_called.get());
    }
}
