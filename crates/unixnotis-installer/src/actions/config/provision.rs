//! Config and theme file creation or reset logic

use std::fs;

use anyhow::{anyhow, Context, Result};
use unixnotis_core::Config;

use crate::paths::format_with_home;

use super::super::{log_line, ActionContext};
use super::backup::{
    backup_existing_file, create_backup_dir, ensure_installer_config, load_installer_config,
    write_atomic,
};

pub(crate) fn ensure_config(ctx: &mut ActionContext) -> Result<()> {
    let config = Config::default();
    let config_dir = Config::default_config_dir().map_err(|err| anyhow!(err.to_string()))?;
    let config_path = Config::default_config_path().map_err(|err| anyhow!(err.to_string()))?;
    log_line(
        ctx,
        format!("Config directory: {}", format_with_home(&config_dir)),
    );

    // Create the config root first so later file writes do not race missing parents
    fs::create_dir_all(&config_dir).with_context(|| "failed to create config directory")?;

    if config_path.exists() {
        log_line(
            ctx,
            format!("Config file present: {}", format_with_home(&config_path)),
        );
    } else {
        // Write a default config so there is always a working base to edit
        let config_toml = render_default_config_toml(&config)?;
        write_atomic(&config_path, &config_toml).with_context(|| "failed to write config.toml")?;
        log_line(
            ctx,
            format!("Config file created: {}", format_with_home(&config_path)),
        );
    }

    ensure_installer_config(ctx, &config_dir)?;

    let theme_paths = config
        .resolve_theme_paths()
        .map_err(|err| anyhow!(err.to_string()))?;
    let theme_entries = [
        ("base.css", &theme_paths.base_css),
        ("panel.css", &theme_paths.panel_css),
        ("popup.css", &theme_paths.popup_css),
        ("widgets.css", &theme_paths.widgets_css),
        ("media.css", &theme_paths.media_css),
    ];

    let pre_existing = theme_entries
        .iter()
        .map(|(_, path)| path.exists())
        .collect::<Vec<_>>();

    config
        .ensure_theme_files(&theme_paths)
        .map_err(|err| anyhow!(err.to_string()))?;

    for ((name, path), existed) in theme_entries.iter().zip(pre_existing.iter()) {
        let status = if *existed { "present" } else { "created" };
        log_line(
            ctx,
            format!(
                "Theme file {}: {} ({})",
                name,
                status,
                format_with_home(path)
            ),
        );
    }

    Ok(())
}

pub(crate) fn reset_config(ctx: &mut ActionContext) -> Result<()> {
    let config = Config::default();
    let config_dir = Config::default_config_dir().map_err(|err| anyhow!(err.to_string()))?;
    let config_path = Config::default_config_path().map_err(|err| anyhow!(err.to_string()))?;

    fs::create_dir_all(&config_dir).with_context(|| "failed to create config directory")?;
    ensure_installer_config(ctx, &config_dir)?;

    let installer_config = load_installer_config(&config_dir, ctx);
    let backup_dir = create_backup_dir(ctx, &config_dir, installer_config.backups.keep)?;

    // Preserve the live config before writing defaults over it
    backup_existing_file(ctx, &config_path, "config.toml", backup_dir.as_deref())?;

    let config_toml = render_default_config_toml(&config)?;
    write_atomic(&config_path, &config_toml).with_context(|| "failed to write config.toml")?;
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

    write_atomic(&theme_paths.base_css, unixnotis_core::DEFAULT_BASE_CSS)
        .with_context(|| "failed to write base.css")?;
    write_atomic(&theme_paths.panel_css, unixnotis_core::DEFAULT_PANEL_CSS)
        .with_context(|| "failed to write panel.css")?;
    write_atomic(&theme_paths.popup_css, unixnotis_core::DEFAULT_POPUP_CSS)
        .with_context(|| "failed to write popup.css")?;
    write_atomic(
        &theme_paths.widgets_css,
        unixnotis_core::DEFAULT_WIDGETS_CSS,
    )
    .with_context(|| "failed to write widgets.css")?;
    write_atomic(&theme_paths.media_css, unixnotis_core::DEFAULT_MEDIA_CSS)
        .with_context(|| "failed to write media.css")?;

    log_line(
        ctx,
        format!("Reset theme files in {}", format_with_home(&config_dir)),
    );
    Ok(())
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

    if !config_toml.contains(&panel_height_line) {
        return Err(anyhow!("default config template missing panel height line"));
    }

    config_toml = config_toml.replacen(&panel_height_line, &panel_height_block, 1);
    Ok(config_toml)
}
