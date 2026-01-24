//! Config and theme file creation/reset logic.

use std::fs;
use std::path::{Path, PathBuf};

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

    // Preserve existing config before overwriting so customizations are recoverable.
    backup_existing_file(ctx, &config_path, "config.toml")?;

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

    // Backup theme files before reset to avoid accidental loss of user styling.
    backup_existing_file(ctx, &theme_paths.base_css, "base.css")?;
    backup_existing_file(ctx, &theme_paths.panel_css, "panel.css")?;
    backup_existing_file(ctx, &theme_paths.popup_css, "popup.css")?;
    backup_existing_file(ctx, &theme_paths.widgets_css, "widgets.css")?;

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

fn backup_existing_file(ctx: &mut ActionContext, path: &Path, label: &str) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }

    let backup_path = next_backup_path(path);
    fs::rename(path, &backup_path).with_context(|| format!("failed to backup {}", label))?;
    log_line(
        ctx,
        format!(
            "Backed up {} to {}",
            label,
            format_with_home(&backup_path)
        ),
    );
    Ok(())
}

fn next_backup_path(path: &Path) -> PathBuf {
    // Keep backups alongside the original file with .bak suffixes.
    let file_name = path.file_name().unwrap_or_default().to_string_lossy();
    let mut candidate = path.with_file_name(format!("{file_name}.bak"));
    if !candidate.exists() {
        return candidate;
    }

    // Increment suffixes to avoid overwriting prior backups.
    let mut index = 1;
    loop {
        candidate = path.with_file_name(format!("{file_name}.bak.{index}"));
        if !candidate.exists() {
            return candidate;
        }
        index += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::next_backup_path;
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn next_backup_path_increments_suffixes() {
        // Ensures backup naming avoids overwriting existing .bak files.
        let Ok(home) = std::env::var("HOME") else {
            return;
        };
        let dir = PathBuf::from(home).join(".cache").join(format!(
            "unixnotis-installer-backup-test-{}",
            std::process::id()
        ));
        let _ = fs::create_dir_all(&dir);
        let target = dir.join("config.toml");
        let _ = fs::write(&target, "original");
        let first = next_backup_path(&target);
        let _ = fs::write(&first, "bak");
        let second = next_backup_path(&target);
        assert_ne!(first, second);
        let _ = fs::remove_dir_all(&dir);
    }
}
