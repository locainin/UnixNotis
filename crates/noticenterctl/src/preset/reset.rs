//! Non-D-Bus configuration reset frontend

use anyhow::{Context, Result};
use std::io::{self, BufRead, IsTerminal, Write};
use unixnotis_core::{
    ensure_installer_config, load_installer_config, reset_config_to_defaults, Config,
    ResetConfigOptions,
};

pub(super) fn run_reset_config(skip_confirmation: bool) -> Result<()> {
    let stdin = io::stdin();
    // Local reset is destructive, so unattended calls must opt in explicitly
    if !skip_confirmation && !confirm_reset(&mut stdin.lock(), stdin.is_terminal())? {
        println!("Reset cancelled.");
        return Ok(());
    }
    let config_dir =
        Config::default_config_dir().map_err(|error| anyhow::anyhow!(error.to_string()))?;
    // Both local frontends create and read the same settings file
    let _ = ensure_installer_config(&config_dir).context("prepare installer settings")?;
    let retention = load_installer_config(&config_dir)
        .context("load installer settings")?
        .backups
        .keep;
    // The core operation owns all filesystem changes and rollback behavior
    let report = reset_config_to_defaults(&ResetConfigOptions {
        config_dir,
        backup_retention: retention,
    })
    .context("reset configuration to defaults")?;
    if let Some(backup_dir) = report.backup_dir {
        println!(
            "Backed up existing configuration to:\n{}",
            backup_dir.display()
        );
    } else {
        println!("No backup was created because backup retention is disabled.");
    }
    println!("Reset config.toml to current defaults.");
    println!("Reset bundled scripts.");
    println!("Theme source is now embedded stock.");
    Ok(())
}

pub(super) fn confirm_reset(input: &mut impl BufRead, interactive: bool) -> Result<bool> {
    if !interactive {
        return Err(anyhow::anyhow!(
            "reset-config requires --yes when standard input is not interactive"
        ));
    }
    print!(
        "This will reset UnixNotis configuration and bundled scripts.\n\
Existing files will be backed up before replacement.\n\
Continue? [y/N] "
    );
    io::stdout().flush().context("flush reset confirmation")?;
    let mut answer = String::new();
    input
        .read_line(&mut answer)
        .context("read reset confirmation")?;
    Ok(matches!(
        answer.trim().to_ascii_lowercase().as_str(),
        "y" | "yes"
    ))
}

#[cfg(test)]
#[path = "tests/reset.rs"]
mod tests;
