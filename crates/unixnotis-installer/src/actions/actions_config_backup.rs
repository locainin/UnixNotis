//! Backup and restore helpers for installer config operations.
//!
//! Keeps backup logic isolated from the core config-writing routines to
//! simplify maintenance and make retention/restore behavior easier to audit.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use chrono::Local;
use serde::Deserialize;
use unixnotis_core::Config;

use crate::paths::format_with_home;

use super::{log_line, ActionContext};

const INSTALLER_CONFIG_FILE: &str = "installer.toml";
const BACKUP_PREFIX: &str = "Backup-";
const INSTALLER_CONFIG_TEMPLATE: &str = r#"# UnixNotis installer settings
# Backup retention for config/theme resets.
[backups]
keep = 3
"#;

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(super) struct InstallerConfig {
    pub(super) backups: BackupConfig,
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub(super) struct BackupConfig {
    /// Number of backup directories to keep in the config root.
    pub(super) keep: usize,
}

impl Default for BackupConfig {
    fn default() -> Self {
        Self { keep: 3 }
    }
}

pub(super) fn ensure_installer_config(
    ctx: &mut ActionContext,
    config_dir: &Path,
) -> Result<PathBuf> {
    let config_path = config_dir.join(INSTALLER_CONFIG_FILE);
    if config_path.exists() {
        log_line(
            ctx,
            format!(
                "Installer config present: {}",
                format_with_home(&config_path)
            ),
        );
        return Ok(config_path);
    }

    write_atomic(&config_path, INSTALLER_CONFIG_TEMPLATE)
        .with_context(|| "failed to write installer.toml")?;
    log_line(
        ctx,
        format!(
            "Installer config created: {}",
            format_with_home(&config_path)
        ),
    );
    Ok(config_path)
}

pub(super) fn load_installer_config(config_dir: &Path, ctx: &mut ActionContext) -> InstallerConfig {
    let config_path = config_dir.join(INSTALLER_CONFIG_FILE);
    let Ok(contents) = fs::read_to_string(&config_path) else {
        return InstallerConfig::default();
    };
    match toml::from_str(&contents) {
        Ok(config) => config,
        Err(err) => {
            log_line(
                ctx,
                format!(
                    "Warning: invalid installer config at {}: {}",
                    format_with_home(&config_path),
                    err
                ),
            );
            InstallerConfig::default()
        }
    }
}

pub(super) fn create_backup_dir(
    ctx: &mut ActionContext,
    config_dir: &Path,
    keep: usize,
) -> Result<Option<PathBuf>> {
    if keep == 0 {
        log_line(ctx, "Backups disabled (installer.toml keep = 0).");
        return Ok(None);
    }

    // Each reset produces a dedicated backup directory to avoid filename bloat.
    // Format is Backup-YYYY-MM-DD (date-only) with an optional numeric suffix.
    let stamp = backup_stamp_from_date_cmd(ctx)?;
    let base_name = format!("{BACKUP_PREFIX}{stamp}");
    let mut candidate = config_dir.join(base_name);

    // If a backup already exists for the same day, add a zero-padded suffix.
    let mut suffix = 1;
    while candidate.exists() {
        candidate = config_dir.join(format!("{BACKUP_PREFIX}{stamp}-{suffix:03}"));
        suffix += 1;
    }

    fs::create_dir_all(&candidate).with_context(|| "failed to create backup directory")?;
    log_line(
        ctx,
        format!("Backup directory created: {}", format_with_home(&candidate)),
    );

    prune_old_backups(ctx, config_dir, keep)?;
    Ok(Some(candidate))
}

pub(crate) fn list_backup_dirs_for_ui() -> Vec<PathBuf> {
    let Ok(config_dir) = Config::default_config_dir() else {
        return Vec::new();
    };
    let mut backups = list_backup_dirs(&config_dir);
    backups.sort();
    backups
}

pub(super) fn list_backup_dirs(config_dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(config_dir) else {
        return Vec::new();
    };
    entries
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let file_type = entry.file_type().ok()?;
            if !file_type.is_dir() {
                return None;
            }
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if !name.starts_with(BACKUP_PREFIX) {
                return None;
            }
            Some(entry.path())
        })
        .collect()
}

fn prune_old_backups(ctx: &mut ActionContext, config_dir: &Path, keep: usize) -> Result<()> {
    if keep == 0 {
        return Ok(());
    }

    let mut backups = list_backup_dirs(config_dir);
    // Lexicographic sort works with YYYY-MM-DD names and zero-padded suffixes.
    backups.sort();

    if backups.len() <= keep {
        return Ok(());
    }

    let excess = backups.len().saturating_sub(keep);
    for path in backups.into_iter().take(excess) {
        if let Err(err) = fs::remove_dir_all(&path) {
            log_line(
                ctx,
                format!(
                    "Warning: failed to remove old backup {}: {}",
                    format_with_home(&path),
                    err
                ),
            );
        } else {
            log_line(
                ctx,
                format!("Removed old backup {}", format_with_home(&path)),
            );
        }
    }

    Ok(())
}

pub(super) fn backup_existing_file(
    ctx: &mut ActionContext,
    path: &Path,
    label: &str,
    backup_dir: Option<&Path>,
) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }

    let Some(backup_dir) = backup_dir else {
        return Ok(());
    };

    let file_name = path.file_name().unwrap_or_default().to_string_lossy();
    let backup_path = backup_dir.join(file_name.as_ref());
    // Copy first so the original remains intact until new content is written.
    // This avoids leaving users without a live config if a later write fails.
    fs::copy(path, &backup_path).with_context(|| format!("failed to backup {}", label))?;
    log_line(
        ctx,
        format!("Backed up {} to {}", label, format_with_home(&backup_path)),
    );
    Ok(())
}

pub(crate) fn restore_config(ctx: &mut ActionContext) -> Result<()> {
    let Some(backup_dir) = ctx.restore_backup.clone() else {
        return Err(anyhow!("no backup directory selected"));
    };

    // Derive config root from the selected backup to avoid env races during tests.
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

    // Restore config.toml first so theme paths resolve using the restored settings.
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
        if let Some(parent) = target.parent() {
            // Create parent directories for custom theme paths before restoring content.
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

fn backup_stamp_from_date_cmd(ctx: &mut ActionContext) -> Result<String> {
    let system_stamp = backup_stamp_from_system_time()?;
    let output = std::process::Command::new("date")
        .arg("+%Y-%m-%d")
        .output()
        .with_context(|| "failed to execute date command for backup naming")?;
    if !output.status.success() {
        return Err(anyhow!("date command failed; install coreutils and retry"));
    }
    let stamp = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if stamp.is_empty() || !stamp.chars().all(|ch| ch.is_ascii_digit() || ch == '-') {
        log_line(
            ctx,
            format!(
                "Warning: unexpected date output '{}'; expected YYYY-MM-DD",
                stamp
            ),
        );
        return Err(anyhow!("invalid date output for backup naming"));
    }
    if stamp != system_stamp {
        log_line(
            ctx,
            format!(
                "Warning: date output '{}' differs from system time '{}'; using system time",
                stamp, system_stamp
            ),
        );
        return Ok(system_stamp);
    }
    Ok(stamp)
}

fn backup_stamp_from_system_time() -> Result<String> {
    // Use chrono for a safe, dependency-supported YYYY-MM-DD stamp.
    Ok(Local::now().format("%Y-%m-%d").to_string())
}

pub(super) fn write_atomic(path: &Path, contents: &str) -> std::io::Result<()> {
    // Write to a sibling temp file, then rename to avoid partial writes.
    let file_name = path.file_name().unwrap_or_default().to_string_lossy();
    let temp_name = format!("{file_name}.tmp-{}", std::process::id());
    let temp_path = path.with_file_name(temp_name);

    if temp_path.exists() {
        let _ = fs::remove_file(&temp_path);
    }

    fs::write(&temp_path, contents)?;
    fs::rename(&temp_path, path).inspect_err(|_err| {
        let _ = fs::remove_file(&temp_path);
    })
}

#[cfg(test)]
mod tests {
    use super::{list_backup_dirs, prune_old_backups, BackupConfig};
    use crate::detect::Detection;
    use crate::events::UiMessage;
    use crate::model::ActionMode;
    use crate::paths::InstallPaths;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::mpsc;

    #[test]
    fn prune_old_backups_keeps_newest() {
        // Ensures backup pruning keeps only the newest N directories by name.
        let root = PathBuf::from("target").join(format!(
            "unixnotis-installer-backup-prune-test-{}",
            std::process::id()
        ));
        let _ = fs::create_dir_all(&root);
        let names = [
            "Backup-2024-01-01",
            "Backup-2024-01-02",
            "Backup-2024-01-03",
            "Backup-2024-01-04",
        ];
        for name in names {
            let _ = fs::create_dir_all(root.join(name));
        }

        let detection = Detection {
            owner: None,
            daemons: Vec::new(),
        };
        let paths = InstallPaths::discover().expect("paths should resolve in repo tests");
        let (tx, _rx) = mpsc::sync_channel::<UiMessage>(8);
        let mut ctx = crate::actions::ActionContext {
            detection: &detection,
            paths: &paths,
            install_state: None,
            log_tx: tx,
            action_mode: ActionMode::Install,
            restore_backup: None,
        };
        prune_old_backups(&mut ctx, &root, 2).expect("prune should succeed");

        let mut remaining = list_backup_dirs(&root)
            .into_iter()
            .map(|path| path.file_name().unwrap().to_string_lossy().to_string())
            .collect::<Vec<_>>();
        remaining.sort();
        assert_eq!(
            remaining,
            vec![
                "Backup-2024-01-03".to_string(),
                "Backup-2024-01-04".to_string()
            ]
        );

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn backup_config_defaults_to_three() {
        let config = BackupConfig::default();
        assert_eq!(config.keep, 3);
    }

    #[test]
    fn restore_config_uses_restored_theme_paths() {
        let root = PathBuf::from("target").join(format!(
            "unixnotis-installer-restore-test-{}",
            std::process::id()
        ));
        let config_dir = root.join("unixnotis");
        let _ = fs::create_dir_all(&config_dir);
        let backup_dir = config_dir.join("Backup-2024-01-01");
        let _ = fs::create_dir_all(&backup_dir);

        let config_toml = r#"
[theme]
base_css = "themes/custom/base.css"
panel_css = "themes/custom/panel.css"
popup_css = "themes/custom/popup.css"
widgets_css = "themes/custom/widgets.css"
"#;
        fs::write(backup_dir.join("config.toml"), config_toml).expect("write config");
        fs::write(backup_dir.join("base.css"), "base").expect("write base");
        fs::write(backup_dir.join("panel.css"), "panel").expect("write panel");
        fs::write(backup_dir.join("popup.css"), "popup").expect("write popup");
        fs::write(backup_dir.join("widgets.css"), "widgets").expect("write widgets");

        let detection = Detection {
            owner: None,
            daemons: Vec::new(),
        };
        let paths = InstallPaths::discover().expect("paths should resolve in repo tests");
        let (tx, _rx) = mpsc::sync_channel::<UiMessage>(8);
        let mut ctx = crate::actions::ActionContext {
            detection: &detection,
            paths: &paths,
            install_state: None,
            log_tx: tx,
            action_mode: ActionMode::Install,
            restore_backup: Some(backup_dir.clone()),
        };

        super::restore_config(&mut ctx).expect("restore should succeed");

        let config_path = config_dir.join("config.toml");
        assert!(config_path.exists());
        let custom_base = config_dir.join("themes").join("custom").join("base.css");
        let custom_panel = config_dir.join("themes").join("custom").join("panel.css");
        let custom_popup = config_dir.join("themes").join("custom").join("popup.css");
        let custom_widgets = config_dir.join("themes").join("custom").join("widgets.css");
        assert!(custom_base.exists());
        assert!(custom_panel.exists());
        assert!(custom_popup.exists());
        assert!(custom_widgets.exists());

        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_dir_all(&root);
    }
}
