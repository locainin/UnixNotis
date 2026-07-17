//! Filesystem validation helpers for preset import and export
//!
//! These checks keep path validation separate from the actual disk read and
//! write helpers so the guard logic stays easier to review

use anyhow::{anyhow, Context, Result};
#[cfg(target_os = "linux")]
use rustix::fs::fstat;
use std::fs;
#[cfg(target_os = "linux")]
use std::os::fd::OwnedFd;
#[cfg(target_os = "linux")]
use std::os::unix::fs::MetadataExt;
use std::path::{Component, Path, PathBuf};

use super::super::pathing::normalize_relative_path;

pub fn ensure_safe_target_path(config_dir: &Path, relative_path: &Path) -> Result<PathBuf> {
    // Normalize first so later checks only deal with one clean relative path form
    let relative_path = normalize_relative_path(relative_path)?;
    // Join stays local only if every live path segment under the root is a real directory
    let target_path = config_dir.join(&relative_path);

    // Existing symlink components could redirect writes outside the config tree
    let mut probe = config_dir.to_path_buf();
    for component in relative_path.components() {
        if let Component::Normal(part) = component {
            probe.push(part);
            if !probe.exists() {
                // A missing tail segment is fine because nothing can redirect through it yet
                break;
            }

            let metadata = fs::symlink_metadata(&probe)
                .with_context(|| format!("inspect target path {}", probe.display()))?;
            if metadata.file_type().is_symlink() {
                // Reject the whole import target once a single segment can jump outside the root
                return Err(anyhow!(
                    "preset import blocked because this path leaves the UnixNotis config directory through a symlink: {}",
                    probe.display()
                ));
            }
            if probe != target_path && !metadata.is_dir() {
                // A regular file cannot act as a parent even though it cannot escape the root
                return Err(anyhow!(
                    "preset import target has a non-directory parent component: {}",
                    probe.display()
                ));
            }
        }
    }

    Ok(target_path)
}

pub fn ensure_no_symlink_ancestors(path: &Path) -> Result<()> {
    // A symlink anywhere on the live config root path can redirect all later writes
    let mut probe = PathBuf::new();
    for component in path.components() {
        match component {
            // Keep any drive prefix intact on platforms that use one
            Component::Prefix(prefix) => probe.push(prefix.as_os_str()),
            // Rebuild the absolute path one segment at a time from the filesystem root
            Component::RootDir => probe.push(Path::new("/")),
            // `.` has no effect on the real target path
            Component::CurDir => {}
            Component::ParentDir => {
                // Parent segments here would make the root check itself ambiguous
                return Err(anyhow!(
                    "path contains unexpected parent traversal: {}",
                    path.display()
                ));
            }
            // Normal path parts are checked one by one so a linked parent cannot hide deeper hops
            Component::Normal(part) => probe.push(part),
        }

        if !probe.exists() {
            // Once a component does not exist yet, later symlink checks cannot inspect deeper
            break;
        }

        let metadata = fs::symlink_metadata(&probe)
            .with_context(|| format!("inspect path component {}", probe.display()))?;
        if metadata.file_type().is_symlink() {
            // Stop before import writes into a root that already points somewhere else
            return Err(anyhow!(
                "preset import blocked because the UnixNotis config directory path goes through a symlink: {}",
                probe.display()
            ));
        }
    }

    Ok(())
}

#[cfg(target_os = "linux")]
pub fn ensure_dir_fd_matches_live_path(root_dir: &OwnedFd, live_path: &Path) -> Result<()> {
    let live_metadata = fs::metadata(live_path)
        .with_context(|| format!("inspect live config directory {}", live_path.display()))?;
    let fd_stat = fstat(root_dir)
        .with_context(|| format!("stat open config directory {}", live_path.display()))?;

    // If the opened dir inode no longer matches the visible path, later writes would land elsewhere
    if live_metadata.dev() != fd_stat.st_dev || live_metadata.ino() != fd_stat.st_ino {
        return Err(anyhow!(
            "preset import blocked because the UnixNotis config directory changed during import: {}",
            live_path.display()
        ));
    }

    Ok(())
}
