//! Config-root helpers for preset export and import
//!
//! This module stays focused on the live `UnixNotis` config tree:
//! walking files for export and filtering out internal snapshot directories

use anyhow::{anyhow, Context, Result};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::os::fd::OwnedFd;
use std::path::{Path, PathBuf};

use super::archive::{
    MAX_PRESET_FILE_BYTES, MAX_PRESET_PAYLOAD_FILES, MAX_PRESET_TOTAL_PAYLOAD_BYTES,
};
use super::filesystem::read_relative_file_secure_bounded;
use super::pathing::{normalize_relative_path, relative_path_matches_exclusion};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PresetFileSource {
    // Relative path as it should appear in the bundle and on import
    pub(super) relative_path: PathBuf,
    // Real on-disk source path used while export streams files into the archive
    pub(super) source_path: PathBuf,
    // Bytes are captured from the same secure descriptor used for metadata validation
    pub(super) source_contents: Vec<u8>,
    // Cached size goes into the manifest so later validation stays cheap
    pub(super) size: u64,
    // File mode is cached so archive overrides can keep the same permissions
    pub(super) mode: u32,
    // Export can replace config.toml bytes in memory without touching the live tree
    pub(super) contents_override: Option<Vec<u8>>,
}

#[derive(Debug)]
pub(super) struct SecureFileCapture {
    // Bytes and mode come from one descriptor so later collection cannot observe a replacement file
    pub(super) contents: Vec<u8>,
    pub(super) mode: u32,
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

pub(super) fn collect_selected_config_files_from_root(
    root_fd: &OwnedFd,
    config_dir: &Path,
    relative_paths: &[PathBuf],
    output_path: Option<&Path>,
    exclusions: &[PathBuf],
    captures: &BTreeMap<PathBuf, SecureFileCapture>,
) -> Result<CollectedConfigFiles> {
    // Callers with an existing snapshot keep every read pinned to one verified directory
    let output_path = output_path.map(resolve_working_path).transpose()?;
    let mut collected = CollectedConfigFiles::default();
    let mut total_bytes = 0u64;

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

        let (source_contents, descriptor_mode) = if let Some(capture) = captures.get(&relative) {
            // Dependency scanning already captured this exact file through the secure root descriptor
            (capture.contents.clone(), capture.mode)
        } else {
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

            // Secure descriptor-relative reading closes the validation-to-read race
            read_relative_file_secure_bounded(root_fd, &relative, MAX_PRESET_FILE_BYTES)?
        };
        total_bytes = checked_export_total(
            total_bytes,
            source_contents.len() as u64,
            collected.files.len(),
        )?;
        let mode = file_mode(&path, descriptor_mode)?;
        collected.files.push(PresetFileSource {
            relative_path: relative,
            source_path: path,
            size: source_contents.len() as u64,
            mode,
            source_contents,
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

pub(super) fn checked_export_total(current: u64, file_size: u64, file_count: usize) -> Result<u64> {
    if file_count >= MAX_PRESET_PAYLOAD_FILES {
        return Err(anyhow!(
            "preset export selects more than {MAX_PRESET_PAYLOAD_FILES} files"
        ));
    }
    current
        .checked_add(file_size)
        .filter(|total| *total <= MAX_PRESET_TOTAL_PAYLOAD_BYTES)
        .ok_or_else(|| {
            anyhow!("preset export payload exceeds {MAX_PRESET_TOTAL_PAYLOAD_BYTES} bytes")
        })
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

fn file_mode(path: &Path, raw_mode: u32) -> Result<u32> {
    #[cfg(unix)]
    {
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
        let _ = raw_mode;
        Ok(0o644)
    }
}
