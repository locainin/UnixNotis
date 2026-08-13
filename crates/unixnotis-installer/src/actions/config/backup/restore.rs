//! Transactional backup restore planning and commit

use std::collections::HashSet;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use unixnotis_core::filesystem::open_regular_file;
use unixnotis_core::{Config, DEFAULT_SCRIPTS, MAX_CONFIG_BYTES};

use crate::paths::format_with_home;

use super::super::super::{log_line, ActionContext};
use super::listing::BACKUP_PREFIX;

pub(super) const MAX_RESTORE_FILE_BYTES: u64 = 16 * 1024 * 1024;

struct RestorePlan {
    config_path: PathBuf,
    files: Vec<RestoreFile>,
    warnings: Vec<String>,
}

struct RestoreFile {
    label: String,
    target: PathBuf,
    mode: u32,
    contents: Vec<u8>,
}

pub fn restore_config(ctx: &mut ActionContext) -> Result<()> {
    let Some(backup_dir) = ctx.restore_backup.clone() else {
        return Err(anyhow!("no backup directory selected"));
    };
    let config_dir = backup_dir
        .parent()
        .ok_or_else(|| anyhow!("backup directory missing parent"))?
        .to_path_buf();
    validate_backup_directory_name(&backup_dir)?;

    // A durable journal makes an interrupted earlier restore safe before another plan is built
    super::restore_transaction::recover_pending_restore(&config_dir)?;

    log_line(
        ctx,
        format!("Restoring config from {}", format_with_home(&backup_dir)),
    );
    // Planning reads, parses, resolves, and bounds every source before any live file changes
    let plan = build_restore_plan(&backup_dir, &config_dir)?;
    for warning in &plan.warnings {
        log_line(ctx, format!("Warning: {warning}"));
    }
    apply_restore_plan(&plan)?;
    for file in &plan.files {
        log_line(
            ctx,
            format!(
                "Restored {} -> {}",
                file.label,
                format_with_home(&file.target)
            ),
        );
    }
    Ok(())
}

fn validate_backup_directory_name(backup_dir: &Path) -> Result<()> {
    let backup_name = backup_dir
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    if !backup_name.starts_with(BACKUP_PREFIX) {
        return Err(anyhow!("backup directory name is not recognized"));
    }
    Ok(())
}

fn build_restore_plan(backup_dir: &Path, config_dir: &Path) -> Result<RestorePlan> {
    let config_path = config_dir.join("config.toml");
    let backup_config = backup_dir.join("config.toml");
    let mut files = Vec::new();
    let mut warnings = Vec::new();

    let (config, config_restore) = if backup_entry_exists(&backup_config)? {
        let contents = read_backup_file_bounded(&backup_config, MAX_CONFIG_BYTES)
            .context("failed to read backup config.toml")?;
        let text = std::str::from_utf8(&contents)
            .map_err(|_error| anyhow!("backup config.toml is not valid UTF-8"))?;
        // Parser details may contain private configuration values, so the public error stays stable
        let config = Config::parse(text)
            .map_err(|_error| anyhow!("backup config.toml is not valid schema v5"))?;
        (
            config,
            Some(RestoreFile {
                label: "config.toml".to_string(),
                target: config_path.clone(),
                mode: 0o644,
                contents,
            }),
        )
    } else if backup_entry_exists(&config_path)? {
        warnings.push("backup missing config.toml; leaving current file unchanged".to_string());
        (
            Config::load_from_path(&config_path)
                .map_err(|_error| anyhow!("live config.toml is not valid schema v5"))?,
            None,
        )
    } else {
        warnings.push("backup missing config.toml; leaving current file unchanged".to_string());
        (Config::default(), None)
    };

    let theme_paths = config
        .resolve_theme_paths_from(config_dir)
        .map_err(|error| anyhow!(error.to_string()))?;
    let theme_targets = [
        ("base.css", theme_paths.base_css),
        ("panel.css", theme_paths.panel_css),
        ("popup.css", theme_paths.popup_css),
        ("widgets.css", theme_paths.widgets_css),
        ("media.css", theme_paths.media_css),
    ];
    for (name, target) in theme_targets {
        plan_optional_file(
            &mut files,
            &mut warnings,
            backup_dir,
            config_dir,
            name,
            target,
            0o644,
        )?;
    }

    for script in DEFAULT_SCRIPTS {
        let name = Path::new(script.relative_path)
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| anyhow!("script path has no UTF-8 file name"))?;
        plan_optional_file(
            &mut files,
            &mut warnings,
            backup_dir,
            config_dir,
            script.relative_path,
            config_dir.join(script.relative_path),
            0o755,
        )
        .with_context(|| format!("plan restore for {name}"))?;
    }

    if let Some(config_restore) = config_restore {
        // Config is the final visibility switch after every referenced payload is durable
        files.push(config_restore);
    }

    reject_duplicate_targets(&files)?;
    Ok(RestorePlan {
        config_path,
        files,
        warnings,
    })
}

fn plan_optional_file(
    files: &mut Vec<RestoreFile>,
    warnings: &mut Vec<String>,
    backup_dir: &Path,
    config_dir: &Path,
    label: &str,
    target: PathBuf,
    mode: u32,
) -> Result<()> {
    let source_name = Path::new(label)
        .file_name()
        .ok_or_else(|| anyhow!("restore label has no file name"))?;
    let source = backup_dir.join(source_name);
    if !backup_entry_exists(&source)? {
        warnings.push(format!(
            "backup missing {label}; leaving current file unchanged"
        ));
        return Ok(());
    }
    if !is_restore_target_allowed(config_dir, &target) {
        warnings.push(format!(
            "skipped restoring {label} because target escapes config dir ({})",
            format_with_home(&target)
        ));
        return Ok(());
    }
    let contents = read_backup_file_bounded(&source, MAX_RESTORE_FILE_BYTES)
        .with_context(|| format!("failed to read backup {label}"))?;
    files.push(RestoreFile {
        label: label.to_string(),
        target,
        mode,
        contents,
    });
    Ok(())
}

fn reject_duplicate_targets(files: &[RestoreFile]) -> Result<()> {
    let mut targets = HashSet::new();
    for file in files {
        let normalized = normalize_path_for_compare(&file.target);
        if !targets.insert(normalized) {
            return Err(anyhow!(
                "backup maps multiple files to the same live restore target"
            ));
        }
    }
    Ok(())
}

fn apply_restore_plan(plan: &RestorePlan) -> Result<()> {
    let config_dir = plan
        .config_path
        .parent()
        .ok_or_else(|| anyhow!("live config path has no parent directory"))?;
    let writes = plan
        .files
        .iter()
        .map(|file| super::restore_transaction::RestoreWrite {
            label: &file.label,
            target: &file.target,
            mode: file.mode,
            contents: &file.contents,
        })
        .collect::<Vec<_>>();
    super::restore_transaction::apply_restore_transaction(config_dir, &writes, || {
        // Reloading the published config catches an unexpected filesystem race before commit
        if plan.config_path.exists() {
            Config::load_from_path(&plan.config_path)
                .map_err(|_error| anyhow!("restored config.toml failed post-commit validation"))?;
        }
        Ok(())
    })
}

fn backup_entry_exists(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_metadata) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error).with_context(|| format!("inspect {}", path.display())),
    }
}

fn read_backup_file_bounded(path: &Path, max_bytes: u64) -> Result<Vec<u8>> {
    // Pin the object and reject links or special files before reading any payload bytes
    let file = open_regular_file(path).with_context(|| format!("open {}", path.display()))?;
    let size = file
        .metadata()
        .with_context(|| format!("inspect {}", path.display()))?
        .len();
    if size > max_bytes {
        return Err(anyhow!(
            "restore file exceeds {max_bytes} bytes: {}",
            path.display()
        ));
    }
    let mut contents = Vec::with_capacity(usize::try_from(size).unwrap_or(usize::MAX));
    file.take(max_bytes.saturating_add(1))
        .read_to_end(&mut contents)
        .with_context(|| format!("read {}", path.display()))?;
    if u64::try_from(contents.len()).unwrap_or(u64::MAX) > max_bytes {
        return Err(anyhow!(
            "restore file grew beyond {max_bytes} bytes: {}",
            path.display()
        ));
    }
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
    // Existing objects are resolved first so an in-tree symlink cannot redirect a restore
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

#[cfg(test)]
#[path = "tests/restore_validation.rs"]
mod validation_tests;
