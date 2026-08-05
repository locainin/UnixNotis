//! Backup restore helpers and path guards

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use unixnotis_core::filesystem::{create_directory_all, open_regular_file, write_file_atomic};
use unixnotis_core::{Config, DEFAULT_SCRIPTS};

use crate::paths::format_with_home;

use super::super::super::{log_line, ActionContext};
use super::listing::BACKUP_PREFIX;

pub fn restore_config(ctx: &mut ActionContext) -> Result<()> {
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

    log_line(
        ctx,
        format!("Restoring config from {}", format_with_home(&backup_dir)),
    );

    // Restore config.toml first so restored theme paths drive the rest of the write targets
    restore_config_file(ctx, &backup_dir, &config_path)?;
    let config = load_restored_config(ctx, &config_path);
    let theme_paths = config
        .resolve_theme_paths_from(&config_dir)
        .map_err(|err| anyhow!(err.to_string()))?;

    restore_theme_files(ctx, &backup_dir, &config_dir, &theme_paths)?;
    restore_bundled_scripts(ctx, &backup_dir, &config_dir)?;

    Ok(())
}

fn restore_config_file(
    ctx: &mut ActionContext,
    backup_dir: &Path,
    config_path: &Path,
) -> Result<()> {
    let source = backup_dir.join("config.toml");
    if !source.exists() {
        log_line(
            ctx,
            "Warning: backup missing config.toml; leaving current file unchanged".to_string(),
        );
        return Ok(());
    }

    let contents =
        fs::read_to_string(&source).with_context(|| "failed to read backup config.toml")?;
    write_file_atomic(config_path, contents.as_bytes(), 0o644)
        .with_context(|| "failed to restore config.toml")?;
    log_line(
        ctx,
        format!("Restored config.toml -> {}", format_with_home(config_path)),
    );
    Ok(())
}

fn load_restored_config(ctx: &mut ActionContext, config_path: &Path) -> Config {
    if !config_path.exists() {
        return Config::default();
    }
    match Config::load_from_path(config_path) {
        Ok(config) => config,
        Err(err) => {
            log_line(
                ctx,
                format!("Warning: failed to parse restored config.toml ({err:?}); using defaults"),
            );
            Config::default()
        }
    }
}

fn restore_theme_files(
    ctx: &mut ActionContext,
    backup_dir: &Path,
    config_dir: &Path,
    theme_paths: &unixnotis_core::ThemePaths,
) -> Result<()> {
    let theme_targets = [
        ("base.css", &theme_paths.base_css),
        ("panel.css", &theme_paths.panel_css),
        ("popup.css", &theme_paths.popup_css),
        ("widgets.css", &theme_paths.widgets_css),
        ("media.css", &theme_paths.media_css),
    ];
    for (name, target) in theme_targets {
        restore_theme_file(ctx, backup_dir, config_dir, name, target)?;
    }
    Ok(())
}

fn restore_theme_file(
    ctx: &mut ActionContext,
    backup_dir: &Path,
    config_dir: &Path,
    name: &str,
    target: &Path,
) -> Result<()> {
    let source = backup_dir.join(name);
    if !source.exists() {
        log_line(
            ctx,
            format!("Warning: backup missing {name}; leaving current file unchanged"),
        );
        return Ok(());
    }
    if !is_restore_target_allowed(config_dir, target) {
        log_line(
            ctx,
            format!(
                "Warning: skipped restoring {name} because target escapes config dir ({})",
                format_with_home(target)
            ),
        );
        return Ok(());
    }
    let contents =
        fs::read_to_string(&source).with_context(|| format!("failed to read backup {name}"))?;
    write_file_atomic(target, contents.as_bytes(), 0o644)
        .with_context(|| format!("failed to restore {name}"))?;
    log_line(
        ctx,
        format!("Restored {name} -> {}", format_with_home(target)),
    );
    Ok(())
}

fn restore_bundled_scripts(
    ctx: &mut ActionContext,
    backup_dir: &Path,
    config_dir: &Path,
) -> Result<()> {
    // Script backups use their basename because reset stores them directly in the backup root
    for script in DEFAULT_SCRIPTS {
        restore_bundled_script(ctx, backup_dir, config_dir, script)?;
    }
    Ok(())
}

fn restore_bundled_script(
    ctx: &mut ActionContext,
    backup_dir: &Path,
    config_dir: &Path,
    script: &unixnotis_core::DefaultScript,
) -> Result<()> {
    let script_name = Path::new(script.relative_path)
        .file_name()
        .ok_or_else(|| anyhow!("script path has no file name"))?;
    let source = backup_dir.join(script_name);
    if !source.exists() {
        log_line(
            ctx,
            format!(
                "Warning: backup missing {}; leaving current file unchanged",
                script.relative_path
            ),
        );
        return Ok(());
    }

    let target = config_dir.join(script.relative_path);
    if !is_restore_target_allowed(config_dir, &target) {
        log_line(
            ctx,
            format!(
                "Warning: skipped restoring {} because target escapes config dir ({})",
                script.relative_path,
                format_with_home(&target)
            ),
        );
        return Ok(());
    }
    if let Some(parent) = target.parent() {
        create_directory_all(parent, 0o700)
            .with_context(|| format!("create script restore directory {}", parent.display()))?;
    }
    let contents = read_backup_file(&source)
        .with_context(|| format!("failed to read backup {}", script.relative_path))?;
    write_file_atomic(&target, &contents, 0o755)
        .with_context(|| format!("failed to restore {}", script.relative_path))?;
    log_line(
        ctx,
        format!(
            "Restored {} -> {}",
            script.relative_path,
            format_with_home(&target)
        ),
    );
    Ok(())
}

fn read_backup_file(path: &Path) -> Result<Vec<u8>> {
    // Pin the backup object and reject links or special files before reading it
    let mut file =
        open_regular_file(path).with_context(|| format!("open backup file {}", path.display()))?;
    let mut contents = Vec::new();
    file.read_to_end(&mut contents)
        .with_context(|| format!("read backup file {}", path.display()))?;
    Ok(contents)
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
        std::env::current_dir().map_or_else(
            |_error| path.to_path_buf(),
            |current_dir| current_dir.join(path),
        )
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
