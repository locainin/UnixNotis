//! Backup restore helpers and path guards

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use unixnotis_core::Config;

use crate::paths::format_with_home;

use super::super::super::{log_line, ActionContext};
use super::retention::BACKUP_PREFIX;
use super::write::write_atomic;

pub(crate) fn restore_config(ctx: &mut ActionContext) -> Result<()> {
    let Some(backup_dir) = ctx.restore_backup.clone() else {
        return Err(anyhow!("no backup directory selected"));
    };

    // Derive the config root from the selected backup so tests do not depend on env state
    let config_dir = backup_dir
        .parent()
        .ok_or_else(|| anyhow!("backup directory missing parent"))?
        .to_path_buf();
    let config_path = config_dir.join("config.toml");

    let backup_name = backup_dir
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    if !backup_name.starts_with(BACKUP_PREFIX) {
        return Err(anyhow!("backup directory name is not recognized"));
    }

    fs::create_dir_all(&config_dir).with_context(|| "failed to create config directory")?;
    log_line(
        ctx,
        format!("Restoring config from {}", format_with_home(&backup_dir)),
    );

    // Restore config.toml first so restored theme paths drive the rest of the write targets
    let config_backup = backup_dir.join("config.toml");
    if config_backup.exists() {
        let contents = fs::read_to_string(&config_backup)
            .with_context(|| "failed to read backup config.toml")?;
        write_atomic(&config_path, &contents).with_context(|| "failed to restore config.toml")?;
        log_line(
            ctx,
            format!("Restored config.toml -> {}", format_with_home(&config_path)),
        );
    } else {
        log_line(
            ctx,
            "Warning: backup missing config.toml; leaving current file unchanged".to_string(),
        );
    }

    let config = if config_path.exists() {
        match Config::load_from_path(&config_path) {
            Ok(config) => config,
            Err(err) => {
                log_line(
                    ctx,
                    format!(
                        "Warning: failed to parse restored config.toml ({:?}); using defaults",
                        err
                    ),
                );
                Config::default()
            }
        }
    } else {
        Config::default()
    };
    let theme_paths = config
        .resolve_theme_paths_from(&config_dir)
        .map_err(|err| anyhow!(err.to_string()))?;

    let theme_targets = [
        ("base.css", theme_paths.base_css),
        ("panel.css", theme_paths.panel_css),
        ("popup.css", theme_paths.popup_css),
        ("widgets.css", theme_paths.widgets_css),
        ("media.css", theme_paths.media_css),
    ];

    for (name, target) in theme_targets {
        let source = backup_dir.join(name);
        if !source.exists() {
            log_line(
                ctx,
                format!(
                    "Warning: backup missing {}; leaving current file unchanged",
                    name
                ),
            );
            continue;
        }
        if !is_restore_target_allowed(&config_dir, &target) {
            log_line(
                ctx,
                format!(
                    "Warning: skipped restoring {} because target escapes config dir ({})",
                    name,
                    format_with_home(&target)
                ),
            );
            continue;
        }
        if let Some(parent) = target.parent() {
            // Create parents for custom theme paths before writing restored content
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create parent dir for {}", name))?;
        }
        let contents = fs::read_to_string(&source)
            .with_context(|| format!("failed to read backup {}", name))?;
        write_atomic(&target, &contents).with_context(|| format!("failed to restore {}", name))?;
        log_line(
            ctx,
            format!("Restored {} -> {}", name, format_with_home(&target)),
        );
    }

    Ok(())
}

pub(in crate::actions::config::backup) fn is_restore_target_allowed(
    config_dir: &Path,
    target: &Path,
) -> bool {
    let base = normalize_path_for_compare(config_dir);
    let target = normalize_path_for_compare(target);
    target.starts_with(&base)
}

fn normalize_path_for_compare(path: &Path) -> PathBuf {
    // Canonicalize when possible, then fall back to lexical cleanup for missing paths
    if let Ok(canonical) = fs::canonicalize(path) {
        return canonical;
    }
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        match std::env::current_dir() {
            Ok(current_dir) => current_dir.join(path),
            Err(_) => path.to_path_buf(),
        }
    };
    if let Ok(canonical) = fs::canonicalize(&absolute) {
        return canonical;
    }
    if let Some(parent) = absolute.parent() {
        if let Ok(parent_canonical) = fs::canonicalize(parent) {
            if let Some(name) = absolute.file_name() {
                return parent_canonical.join(name);
            }
            return parent_canonical;
        }
    }

    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}
