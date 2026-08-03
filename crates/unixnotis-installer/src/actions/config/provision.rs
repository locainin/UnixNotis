//! Config and theme file creation or reset logic

use std::path::Path;

use anyhow::{anyhow, Context, Result};
use unixnotis_core::{
    filesystem::write_file_atomic, render_default_config_toml, reset_config_to_defaults, Config,
    ResetConfigOptions,
};

use crate::paths::format_with_home;

use super::super::{log_line, ActionContext};
use super::backup::{ensure_installer_config, load_installer_config};

pub fn ensure_config(ctx: &mut ActionContext) -> Result<()> {
    let config = Config::default();
    let config_dir = Config::default_config_dir().map_err(|err| anyhow!(err.to_string()))?;
    let config_path = Config::default_config_path().map_err(|err| anyhow!(err.to_string()))?;
    log_line(
        ctx,
        format!("Config directory: {}", format_with_home(&config_dir)),
    );

    if config_path.exists() {
        log_line(
            ctx,
            format!("Config file present: {}", format_with_home(&config_path)),
        );
    } else {
        // Write a default config so there is always a working base to edit
        let config_toml = render_default_config_toml(&config)?;
        write_file_atomic(&config_path, config_toml.as_bytes(), 0o644)
            .with_context(|| "failed to write config.toml")?;
        log_line(
            ctx,
            format!("Config file created: {}", format_with_home(&config_path)),
        );
    }

    ensure_installer_config(ctx, &config_dir)?;
    ensure_default_scripts(ctx, &config_dir)?;

    // New installations use embedded stock CSS until a versioned custom theme is installed
    log_line(ctx, "Theme source: embedded stock".to_string());

    Ok(())
}

pub fn reset_config(ctx: &mut ActionContext) -> Result<()> {
    let config_dir = Config::default_config_dir().map_err(|err| anyhow!(err.to_string()))?;
    ensure_installer_config(ctx, &config_dir)?;
    let installer_config = load_installer_config(&config_dir).context("load installer settings")?;
    let report = reset_config_to_defaults(&ResetConfigOptions {
        config_dir: config_dir.clone(),
        backup_retention: installer_config.backups.keep,
    })
    .context("reset configuration to defaults")?;
    if let Some(backup_dir) = report.backup_dir {
        log_line(
            ctx,
            format!(
                "Backed up existing configuration to {}",
                format_with_home(&backup_dir)
            ),
        );
    }
    log_line(
        ctx,
        "Reset config file and bundled scripts to defaults".to_string(),
    );
    log_line(
        ctx,
        format!(
            "Theme source reset to embedded stock; custom files preserved in {}",
            format_with_home(&config_dir)
        ),
    );
    Ok(())
}

fn ensure_default_scripts(ctx: &mut ActionContext, config_dir: &Path) -> Result<()> {
    // Snapshot presence first so logs can say created without duplicating write logic
    let pre_existing = unixnotis_core::DEFAULT_SCRIPTS
        .iter()
        .map(|script| config_dir.join(script.relative_path).exists())
        .collect::<Vec<_>>();

    // Core owns script provisioning because center startup needs the same guarantee
    Config::ensure_default_scripts_in(config_dir).map_err(|err| anyhow!(err.to_string()))?;

    for (script, existed) in unixnotis_core::DEFAULT_SCRIPTS
        .iter()
        .zip(pre_existing.iter())
    {
        let path = config_dir.join(script.relative_path);
        let status = if *existed { "present" } else { "created" };
        log_line(
            ctx,
            format!("Default script {status}: {}", format_with_home(&path)),
        );
    }
    Ok(())
}
