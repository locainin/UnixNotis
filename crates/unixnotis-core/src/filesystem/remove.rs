//! Descriptor-relative removal for regular files and symbolic links

use std::ffi::{OsStr, OsString};
use std::io;
use std::os::unix::ffi::OsStringExt;
use std::path::{Path, PathBuf};

use rustix::fs::{readlinkat, unlinkat, AtFlags};

use super::atomic::{open_parent_existing, sync_directory, validate_existing_target};

/// Result of removing a symbolic link with an expected target
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemoveSymlinkOutcome {
    /// No filesystem entry existed at the requested path
    Missing,
    /// A matching symbolic link was removed
    Removed,
    /// The link remained because its stored target no longer matched
    TargetMismatch(PathBuf),
}

/// Remove a regular file without following links in its path
///
/// # Errors
///
/// Returns an error when a path component is unsafe, the target is not a regular file, or the
/// unlink or parent-directory synchronization fails
pub fn remove_regular_file(path: &Path) -> io::Result<bool> {
    let Some((parent_fd, file_name)) = existing_parent(path)? else {
        return Ok(false);
    };
    match validate_existing_target(&parent_fd, &file_name) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    }

    unlinkat(&parent_fd, &file_name, AtFlags::empty())?;
    sync_directory(parent_fd)?;
    Ok(true)
}

/// Read a symbolic link target without following links in its parent path
///
/// # Errors
///
/// Returns an error when a path component is unsafe, the target is not a symbolic link, or the
/// link cannot be read
pub fn read_symlink(path: &Path) -> io::Result<Option<PathBuf>> {
    let Some((parent_fd, file_name)) = existing_parent(path)? else {
        return Ok(None);
    };
    match read_symlink_at(&parent_fd, &file_name) {
        Ok(target) => Ok(Some(target)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

/// Remove a symbolic link without requiring a specific target
///
/// # Errors
///
/// Returns an error when a path component is unsafe, the target is not a symbolic link, or the
/// unlink or parent-directory synchronization fails
pub fn remove_symlink(path: &Path) -> io::Result<bool> {
    let Some((parent_fd, file_name)) = existing_parent(path)? else {
        return Ok(false);
    };
    match read_symlink_at(&parent_fd, &file_name) {
        Ok(_target) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    }

    unlinkat(&parent_fd, &file_name, AtFlags::empty())?;
    sync_directory(parent_fd)?;
    Ok(true)
}

/// Remove a symbolic link only when its stored target matches exactly
///
/// # Errors
///
/// Returns an error when a path component is unsafe, the target is not a symbolic link, or the
/// unlink or parent-directory synchronization fails
pub fn remove_symlink_if_target(
    path: &Path,
    expected_target: &Path,
) -> io::Result<RemoveSymlinkOutcome> {
    let Some((parent_fd, file_name)) = existing_parent(path)? else {
        return Ok(RemoveSymlinkOutcome::Missing);
    };
    let actual_target = match read_symlink_at(&parent_fd, &file_name) {
        Ok(target) => target,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(RemoveSymlinkOutcome::Missing);
        }
        Err(error) => return Err(error),
    };
    if actual_target != expected_target {
        return Ok(RemoveSymlinkOutcome::TargetMismatch(actual_target));
    }

    unlinkat(&parent_fd, &file_name, AtFlags::empty())?;
    sync_directory(parent_fd)?;
    Ok(RemoveSymlinkOutcome::Removed)
}

fn existing_parent(path: &Path) -> io::Result<Option<(std::os::fd::OwnedFd, OsString)>> {
    match open_parent_existing(path) {
        Ok(parent) => Ok(Some(parent)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

fn read_symlink_at(parent_fd: &std::os::fd::OwnedFd, file_name: &OsStr) -> io::Result<PathBuf> {
    let target = readlinkat(parent_fd, file_name, Vec::new())?;
    Ok(PathBuf::from(OsString::from_vec(target.into_bytes())))
}

#[cfg(test)]
#[path = "../tests/filesystem/remove.rs"]
mod tests;
