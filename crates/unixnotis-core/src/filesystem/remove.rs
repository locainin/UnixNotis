//! Descriptor-relative removal for regular files and symbolic links

use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};

use rustix::fs::{unlinkat, AtFlags};

use super::atomic::validate_existing_target;
use super::directory::{open_parent_existing, sync_directory};
use super::symlink::read_symlink_at;

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
    // Missing parents mean the requested file is already absent
    let Some((parent_fd, file_name)) = existing_parent(path)? else {
        return Ok(false);
    };
    // Final validation distinguishes regular files from links and special objects
    match validate_existing_target(&parent_fd, &file_name) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    }

    // Unlink and directory sync use the same retained parent descriptor
    unlinkat(&parent_fd, &file_name, AtFlags::empty())?;
    sync_directory(&parent_fd)?;
    Ok(true)
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
    // Reading the stored target proves the final entry is a link without following it
    match read_symlink_at(&parent_fd, &file_name) {
        Ok(_target) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    }

    unlinkat(&parent_fd, &file_name, AtFlags::empty())?;
    sync_directory(&parent_fd)?;
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
    // Capture the exact stored bytes before comparing ownership expectations
    let actual_target = match read_symlink_at(&parent_fd, &file_name) {
        Ok(target) => target,
        Err(error) => match error.kind() {
            io::ErrorKind::NotFound => return Ok(RemoveSymlinkOutcome::Missing),
            _ => return Err(error),
        },
    };
    if actual_target != expected_target {
        // Mismatched links are user state and remain untouched
        return Ok(RemoveSymlinkOutcome::TargetMismatch(actual_target));
    }

    // Only an exact target match reaches the unlink boundary
    unlinkat(&parent_fd, &file_name, AtFlags::empty())?;
    sync_directory(&parent_fd)?;
    Ok(RemoveSymlinkOutcome::Removed)
}

fn existing_parent(path: &Path) -> io::Result<Option<(std::os::fd::OwnedFd, OsString)>> {
    // The optional form keeps idempotent removal separate from unsafe-shape failures
    match open_parent_existing(path) {
        Ok(parent) => Ok(Some(parent)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
#[path = "../tests/filesystem/remove.rs"]
mod tests;
