//! Config and theme file creation/reset logic.

use std::fs;

use anyhow::{anyhow, Context, Result};
use unixnotis_core::Config;

use crate::paths::format_with_home;

use super::{log_line, ActionContext};

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
        log_line(
            ctx,
            format!("Config file missing: {}", format_with_home(&config_path)),
        );
    }

    let theme_paths = config
        .resolve_theme_paths()
        .map_err(|err| anyhow!(err.to_string()))?;

    let theme_entries = [
        ("base.css", &theme_paths.base_css),
        ("panel.css", &theme_paths.panel_css),
        ("popup.css", &theme_paths.popup_css),
        ("widgets.css", &theme_paths.widgets_css),
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

pub fn reset_config(ctx: &mut ActionContext) -> Result<()> {
    let config = Config::default();
    let config_dir = Config::default_config_dir().map_err(|err| anyhow!(err.to_string()))?;
    let config_path = Config::default_config_path().map_err(|err| anyhow!(err.to_string()))?;

    fs::create_dir_all(&config_dir).with_context(|| "failed to create config directory")?;

    let config_toml = toml::to_string_pretty(&config).map_err(|err| anyhow!(err.to_string()))?;
    fs::write(&config_path, config_toml).with_context(|| "failed to write config.toml")?;

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

    fs::write(&theme_paths.base_css, unixnotis_core::DEFAULT_BASE_CSS)
        .with_context(|| "failed to write base.css")?;
    fs::write(&theme_paths.panel_css, unixnotis_core::DEFAULT_PANEL_CSS)
        .with_context(|| "failed to write panel.css")?;
    fs::write(&theme_paths.popup_css, unixnotis_core::DEFAULT_POPUP_CSS)
        .with_context(|| "failed to write popup.css")?;
    fs::write(
        &theme_paths.widgets_css,
        unixnotis_core::DEFAULT_WIDGETS_CSS,
    )
    .with_context(|| "failed to write widgets.css")?;

    log_line(
        ctx,
        format!("Reset theme files in {}", format_with_home(&config_dir)),
    );

    Ok(())
}
