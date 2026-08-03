//! Shared, transactional reset of the user configuration and bundled scripts

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};

use crate::filesystem::{
    copy_file_atomic, create_directory_all, remove_directory_tree, remove_regular_file,
    write_file_atomic, CreateDirectoryOutcome,
};
use crate::{
    Config, DEFAULT_BASE_CSS, DEFAULT_MEDIA_CSS, DEFAULT_PANEL_CSS, DEFAULT_POPUP_CSS,
    DEFAULT_SCRIPTS, DEFAULT_WIDGETS_CSS,
};

const BACKUP_PREFIX: &str = "Backup-";
type ResetWriter = dyn Fn(&Path, &[u8], u32) -> std::io::Result<()>;

/// Inputs for a configuration reset
#[derive(Debug, Clone)]
pub struct ResetConfigOptions {
    pub config_dir: PathBuf,
    pub backup_retention: usize,
}

/// Files changed by a reset and the backup made before it
#[derive(Debug, Clone, Default)]
pub struct ResetConfigReport {
    pub backup_dir: Option<PathBuf>,
    pub backed_up_files: Vec<PathBuf>,
    pub written_files: Vec<PathBuf>,
}

#[derive(Debug, Clone)]
struct OriginalFile {
    path: PathBuf,
    contents: Vec<u8>,
    mode: u32,
}

#[derive(Debug, Clone)]
struct ResetTarget {
    path: PathBuf,
    contents: Vec<u8>,
    mode: u32,
}

/// Render the annotated stock configuration used by both installer frontends
///
/// # Errors
///
/// Returns an error when serialization fails or the expected annotated fields
/// are missing from the serialized configuration
pub fn render_default_config_toml(config: &Config) -> Result<String> {
    let mut config_toml = toml::to_string_pretty(config).context("serialize default config")?;
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

/// Reset config and bundled scripts while retaining a recoverable snapshot
///
/// # Errors
///
/// Returns an error when a destination is unsafe, backup or publication fails,
/// or a partial reset cannot be restored
pub fn reset_config_to_defaults(options: &ResetConfigOptions) -> Result<ResetConfigReport> {
    reset_config_to_defaults_with_writer(options, &write_file_atomic)
}

fn reset_config_to_defaults_with_writer(
    options: &ResetConfigOptions,
    write: &ResetWriter,
) -> Result<ResetConfigReport> {
    reset_config_to_defaults_inner(options, write)
}

fn reset_config_to_defaults_inner(
    options: &ResetConfigOptions,
    write: &ResetWriter,
) -> Result<ResetConfigReport> {
    // Build the default once so every generated file uses one consistent schema
    let config = Config::default();
    // Create the parent before validating child destinations
    create_directory_all(&options.config_dir, 0o700)
        .context("create UnixNotis configuration directory")?;
    let theme_paths = config
        .resolve_theme_paths_from(&options.config_dir)
        .map_err(|error| anyhow!(error.to_string()))?;
    let config_path = options.config_dir.join("config.toml");
    let mut paths = vec![config_path.clone()];
    paths.extend([
        theme_paths.base_css.clone(),
        theme_paths.panel_css.clone(),
        theme_paths.popup_css.clone(),
        theme_paths.widgets_css.clone(),
        theme_paths.media_css.clone(),
    ]);
    for script in DEFAULT_SCRIPTS {
        paths.push(options.config_dir.join(script.relative_path));
    }

    // Validate every destination before touching the first file
    let originals = paths
        .iter()
        .filter_map(|path| snapshot_existing_file(path).transpose())
        .collect::<Result<Vec<_>>>()?;
    // The backup is created before any destination is replaced
    let backup_dir = create_backup_dir(&options.config_dir, options.backup_retention)?;
    let mut report = ResetConfigReport {
        backup_dir: backup_dir.clone(),
        ..ResetConfigReport::default()
    };
    if let Some(backup_dir) = &backup_dir {
        for original in &originals {
            let destination = backup_dir.join(
                original
                    .path
                    .file_name()
                    .ok_or_else(|| anyhow!("configuration path has no file name"))?,
            );
            copy_file_atomic(&original.path, &destination)
                .with_context(|| format!("backup {}", original.path.display()))?;
            report.backed_up_files.push(destination);
        }
    }
    // Prune only after the new backup exists, but before any live file changes
    prune_backups(
        &options.config_dir,
        options.backup_retention,
        backup_dir.as_deref(),
    )
    .context("prune configuration backups")?;

    // Render all replacement content before starting publication
    let config_toml = render_default_config_toml(&config)?;
    let mut targets = vec![ResetTarget {
        path: config_path,
        contents: config_toml.into_bytes(),
        mode: 0o644,
    }];
    // Reset every active stylesheet so file-backed loading is immediately usable
    for (path, contents) in [
        (theme_paths.base_css, DEFAULT_BASE_CSS),
        (theme_paths.panel_css, DEFAULT_PANEL_CSS),
        (theme_paths.popup_css, DEFAULT_POPUP_CSS),
        (theme_paths.widgets_css, DEFAULT_WIDGETS_CSS),
        (theme_paths.media_css, DEFAULT_MEDIA_CSS),
    ] {
        targets.push(ResetTarget {
            path,
            contents: contents.as_bytes().to_vec(),
            mode: 0o644,
        });
    }
    for script in DEFAULT_SCRIPTS {
        let path = options.config_dir.join(script.relative_path);
        if let Some(parent) = path.parent() {
            create_directory_all(parent, 0o700)
                .with_context(|| format!("create script directory {}", parent.display()))?;
        }
        targets.push(ResetTarget {
            path,
            contents: script.contents.as_bytes().to_vec(),
            mode: 0o755,
        });
    }

    // Keep the successful targets so a later write can be rolled back
    let mut written = Vec::new();
    for target in &targets {
        if let Err(error) = write(&target.path, &target.contents, target.mode) {
            let rollback_error = rollback_reset(&written, &originals, write);
            let message = format!("write {}: {error}", target.path.display());
            return match rollback_error {
                Ok(()) => Err(anyhow!(message)),
                Err(rollback) => Err(anyhow!("{message}; rollback failed: {rollback}")),
            };
        }
        written.push(target.path.clone());
        report.written_files.push(target.path.clone());
    }
    Ok(report)
}

fn snapshot_existing_file(path: &Path) -> Result<Option<OriginalFile>> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error).with_context(|| format!("inspect {}", path.display())),
    };
    if !metadata.file_type().is_file() {
        return Err(anyhow!(
            "reset target is not a regular file: {}",
            path.display()
        ));
    }
    Ok(Some(OriginalFile {
        path: path.to_path_buf(),
        contents: fs::read(path).with_context(|| format!("read {}", path.display()))?,
        mode: metadata.permissions().mode() & 0o777,
    }))
}

fn rollback_reset(
    written: &[PathBuf],
    originals: &[OriginalFile],
    write: &ResetWriter,
) -> Result<()> {
    let mut failures = Vec::new();
    // Attempt every restoration so one damaged destination does not hide others
    for path in written.iter().rev() {
        let result =
            if let Some(original) = originals.iter().find(|original| original.path == *path) {
                write(&original.path, &original.contents, original.mode)
                    .with_context(|| format!("restore {}", original.path.display()))
            } else {
                remove_regular_file(path)
                    .map(|_| ())
                    .with_context(|| format!("remove {}", path.display()))
            };
        if let Err(error) = result {
            failures.push(format!("{error:#}"));
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(anyhow!(failures.join("; ")))
    }
}

fn create_backup_dir(config_dir: &Path, retention: usize) -> Result<Option<PathBuf>> {
    if retention == 0 {
        return Ok(None);
    }
    // A suffix handles repeated resets within one clock second
    let stamp = chrono::Local::now().format("%Y-%m-%d-%H%M%S");
    let mut candidate = config_dir.join(format!("{BACKUP_PREFIX}{stamp}"));
    let mut suffix = 1_u32;
    loop {
        // Directory creation reserves the name, so concurrent resets cannot choose one path
        match create_directory_all(&candidate, 0o700)
            .context("create configuration backup directory")?
        {
            CreateDirectoryOutcome::TargetCreated => return Ok(Some(candidate)),
            CreateDirectoryOutcome::TargetAlreadyExisted => {
                candidate = config_dir.join(format!("{BACKUP_PREFIX}{stamp}-{suffix:03}"));
                suffix = suffix
                    .checked_add(1)
                    .ok_or_else(|| anyhow!("configuration backup name space exhausted"))?;
            }
        }
    }
}

fn prune_backups(config_dir: &Path, retention: usize, protected: Option<&Path>) -> Result<()> {
    if retention == 0 {
        return Ok(());
    }
    let mut backups = Vec::new();
    for entry in fs::read_dir(config_dir).context("read configuration backup directory")? {
        let entry = entry.context("read configuration backup entry")?;
        let file_type = entry
            .file_type()
            .with_context(|| format!("inspect backup entry {}", entry.path().display()))?;
        if file_type.is_dir()
            && entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.starts_with(BACKUP_PREFIX))
        {
            backups.push(entry.path());
        }
    }
    // Lexical order matches the timestamped backup names
    backups.sort();
    let excess = backups.len().saturating_sub(retention);
    let mut failures = Vec::new();
    for backup in backups.into_iter().take(excess) {
        if protected.is_some_and(|protected| protected == backup) {
            continue;
        }
        if let Err(error) = remove_directory_tree(&backup) {
            failures.push(format!("{}: {error}", backup.display()));
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(anyhow!(failures.join("; ")))
    }
}

#[cfg(test)]
#[path = "reset/tests/mod.rs"]
mod tests;
