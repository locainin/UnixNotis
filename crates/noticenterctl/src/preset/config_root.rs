//! Config-root helpers for preset export and import
//!
//! This module stays focused on the live `UnixNotis` config tree:
//! walking files for export and filtering out internal snapshot directories

use anyhow::{anyhow, Context, Result};
use std::env;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use super::pathing::{normalize_relative_path, relative_path_matches_exclusion};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PresetFileSource {
    // Relative path as it should appear in the bundle and on import
    pub(super) relative_path: PathBuf,
    // Real on-disk source path used while export streams files into the archive
    pub(super) source_path: PathBuf,
    // Cached size goes into the manifest so later validation stays cheap
    pub(super) size: u64,
    // File mode is cached so archive overrides can keep the same permissions
    pub(super) mode: u32,
    // Export can replace config.toml bytes in memory without touching the live tree
    pub(super) contents_override: Option<Vec<u8>>,
}

#[derive(Debug, Default)]
pub(super) struct CollectedConfigFiles {
    // Portable regular files that should go into the bundle
    pub(super) files: Vec<PresetFileSource>,
    // Symlinks are skipped because they do not round-trip safely across machines
    pub(super) skipped_symlinks: Vec<PathBuf>,
    // Sockets and special files are skipped for the same portability reason
    pub(super) skipped_non_regular: Vec<PathBuf>,
}

pub(super) fn collect_selected_config_files(
    config_dir: &Path,
    relative_paths: &[PathBuf],
    output_path: Option<&Path>,
    exclusions: &[PathBuf],
) -> Result<CollectedConfigFiles> {
    // Export follows an explicit dependency list so unrelated private files never enter a bundle
    let canonical_root = fs::canonicalize(config_dir)
        .with_context(|| format!("resolve config directory {}", config_dir.display()))?;
    let output_path = output_path.map(resolve_working_path).transpose()?;
    let mut collected = CollectedConfigFiles::default();

    for relative_path in relative_paths {
        let relative = normalize_relative_path(relative_path)?;
        if relative_path_matches_exclusion(&relative, exclusions) {
            continue;
        }

        let path = config_dir.join(&relative);
        if output_path.as_ref().is_some_and(|output| *output == path) {
            // A dependency must never make the bundle capture its own output
            continue;
        }

        let metadata = fs::symlink_metadata(&path)
            .with_context(|| format!("read selected config file {}", path.display()))?;
        if metadata.file_type().is_symlink() {
            // Referenced symlinks stay visible in the export summary without being followed
            collected.skipped_symlinks.push(relative);
            continue;
        }
        if !metadata.is_file() {
            // Directories, sockets, and devices are not portable preset dependencies
            collected.skipped_non_regular.push(relative);
            continue;
        }

        let canonical = fs::canonicalize(&path)
            .with_context(|| format!("resolve selected config file {}", path.display()))?;
        if !canonical.starts_with(&canonical_root) {
            return Err(anyhow!(
                "selected config file leaves the config root: {}",
                path.display()
            ));
        }

        let mode = file_mode(&path, &metadata)?;
        collected.files.push(PresetFileSource {
            relative_path: relative,
            source_path: path,
            size: metadata.len(),
            mode,
            contents_override: None,
        });
    }

    collected
        .files
        .sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    collected
        .files
        .dedup_by(|left, right| left.relative_path == right.relative_path);
    collected.skipped_symlinks.sort();
    collected.skipped_symlinks.dedup();
    collected.skipped_non_regular.sort();
    collected.skipped_non_regular.dedup();
    Ok(collected)
}

pub(super) fn override_collected_file_contents(
    collected: &mut CollectedConfigFiles,
    relative_path: &Path,
    contents: Vec<u8>,
) -> Result<()> {
    let Some(file) = collected
        .files
        .iter_mut()
        .find(|file| file.relative_path == relative_path)
    else {
        return Err(anyhow!(
            "preset export could not find {} in the collected file set",
            relative_path.display()
        ));
    };

    // The override stays in memory so export can fix bundled config.toml only
    file.size = contents.len() as u64;
    file.contents_override = Some(contents);
    Ok(())
}

fn resolve_working_path(path: &Path) -> Result<PathBuf> {
    // Relative export targets are resolved from the current shell directory
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }

    Ok(env::current_dir()
        .context("resolve current working directory")?
        .join(path))
}

fn file_mode(path: &Path, metadata: &fs::Metadata) -> Result<u32> {
    #[cfg(unix)]
    {
        let raw_mode = metadata.permissions().mode();
        // Reject special permission bits so exported presets do not carry surprising file behavior
        let permission_mode = raw_mode & 0o7777;
        if permission_mode & 0o7000 != 0 {
            return Err(anyhow!(
                "preset export refuses files with special permission bits: {}",
                path.display()
            ));
        }
        Ok(permission_mode & 0o777)
    }

    #[cfg(not(unix))]
    {
        let _ = path;
        let _ = metadata;
        Ok(0o644)
    }
}
