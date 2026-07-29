//! Config and theme file creation or reset logic

use std::path::Path;

use anyhow::{anyhow, Context, Result};
use unixnotis_core::filesystem::write_file_atomic;
use unixnotis_core::Config;

use crate::paths::format_with_home;

use super::super::{log_line, ActionContext};
use super::backup::{
    backup_existing_file, create_backup_dir, ensure_installer_config, load_installer_config,
};

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
    let config = Config::default();
    let config_dir = Config::default_config_dir().map_err(|err| anyhow!(err.to_string()))?;
    let config_path = Config::default_config_path().map_err(|err| anyhow!(err.to_string()))?;

    ensure_installer_config(ctx, &config_dir)?;

    let installer_config = load_installer_config(&config_dir, ctx);
    let backup_dir = create_backup_dir(ctx, &config_dir, installer_config.backups.keep)?;

    // Preserve the live config before writing defaults over it
    backup_existing_file(ctx, &config_path, "config.toml", backup_dir.as_deref())?;

    let config_toml = render_default_config_toml(&config)?;
    write_file_atomic(&config_path, config_toml.as_bytes(), 0o644)
        .with_context(|| "failed to write config.toml")?;
    log_line(
        ctx,
        format!(
            "Reset config file to defaults: {}",
            format_with_home(&config_path)
        ),
    );

    let theme_paths = config
        .resolve_theme_paths()
        .map_err(|err| anyhow!(err.to_string()))?;

    // Backup theme files before reset so user styling is still recoverable
    backup_existing_file(
        ctx,
        &theme_paths.base_css,
        "base.css",
        backup_dir.as_deref(),
    )?;
    backup_existing_file(
        ctx,
        &theme_paths.panel_css,
        "panel.css",
        backup_dir.as_deref(),
    )?;
    backup_existing_file(
        ctx,
        &theme_paths.popup_css,
        "popup.css",
        backup_dir.as_deref(),
    )?;
    backup_existing_file(
        ctx,
        &theme_paths.widgets_css,
        "widgets.css",
        backup_dir.as_deref(),
    )?;
    backup_existing_file(
        ctx,
        &theme_paths.media_css,
        "media.css",
        backup_dir.as_deref(),
    )?;
    backup_existing_file(
        ctx,
        &theme_paths.manifest_path(),
        "theme.toml",
        backup_dir.as_deref(),
    )?;
    backup_default_scripts(ctx, &config_dir, backup_dir.as_deref())?;

    write_default_scripts(&config_dir)?;

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

fn backup_default_scripts(
    ctx: &mut ActionContext,
    config_dir: &Path,
    backup_dir: Option<&Path>,
) -> Result<()> {
    for script in unixnotis_core::DEFAULT_SCRIPTS {
        let path = config_dir.join(script.relative_path);
        backup_existing_file(ctx, &path, script.relative_path, backup_dir)?;
    }
    Ok(())
}

pub(in crate::actions::config) fn write_default_scripts(config_dir: &Path) -> Result<()> {
    Config::write_default_scripts_in(config_dir).map_err(|err| anyhow!(err.to_string()))
}

pub(in crate::actions::config) fn render_default_config_toml(config: &Config) -> Result<String> {
    let mut config_toml = toml::to_string_pretty(config).map_err(|err| anyhow!(err.to_string()))?;
    let panel_height_line = format!("height = {}\n", config.panel.height);
    let panel_height_block = format!(
        "# Vertical size as a percent of usable monitor height after margins\n\
# and reserved work area\n\
height = {}\n\
\n\
# Exact pixel height override for advanced users\n\
# height_override = 1487\n",
        config.panel.height
    );
    let reduced_motion_line = format!("reduced_motion = {}\n", config.panel.reduced_motion);
    let reduced_motion_block = format!(
        "# Disable panel animation and moving text without requiring GTK 4.20\n\
reduced_motion = {}\n",
        config.panel.reduced_motion
    );
    if !config_toml.contains(&panel_height_line) {
        return Err(anyhow!("default config template missing panel height line"));
    }
    if !config_toml.contains(&reduced_motion_line) {
        return Err(anyhow!(
            "default config template missing reduced motion line"
        ));
    }
    config_toml = config_toml.replacen(&panel_height_line, &panel_height_block, 1);
    config_toml = config_toml.replacen(&reduced_motion_line, &reduced_motion_block, 1);
    Ok(config_toml)
}
