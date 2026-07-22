//! Descriptor-relative removal for regular files and symbolic links

use std::ffi::OsString;
use std::io;
use std::os::fd::OwnedFd;
use std::path::{Path, PathBuf};

use rustix::fs::{fstat, statat, unlinkat, AtFlags};

use super::descriptor::{open_parent_existing, sync_directory};
use super::regular::{file_contents_equal, open_regular_file_at, validate_existing_target};
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

/// Result of conditionally removing one exact regular file
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoveExactFileOutcome {
    /// The requested file or its required marker was absent
    Missing,
    /// One retained file did not contain the authorized bytes
    ContentsMismatch,
    /// Every retained file matched and the requested entries were removed
    Removed,
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

/// Remove two same-directory regular files only when both retained payloads match
///
/// This is intended for a shared artifact and its ownership marker. Both files are opened and
/// preflighted through one parent descriptor before either name is unlinked
///
/// # Errors
///
/// Returns an error when paths have different parents, path traversal is unsafe, either target is
/// not a regular file, retained identities change, or durable unlinking fails
pub fn remove_regular_file_pair_if_contents(
    path: &Path,
    expected_contents: &[u8],
    marker_path: &Path,
    expected_marker_contents: &[u8],
) -> io::Result<RemoveExactFileOutcome> {
    if path.parent() != marker_path.parent() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "guarded files must share one parent directory",
        ));
    }
    let Some((parent_fd, file_name)) = existing_parent(path)? else {
        return Ok(RemoveExactFileOutcome::Missing);
    };
    let marker_name = marker_path
        .file_name()
        .filter(|name| !name.is_empty())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "marker has no file name"))?
        .to_os_string();

    let mut file = match open_regular_file_at(&parent_fd, &file_name) {
        Ok(file) => file,
        Err(error) if file_lookup_is_missing(&error) => return Ok(RemoveExactFileOutcome::Missing),
        Err(error) => return Err(error),
    };
    let mut marker = match open_regular_file_at(&parent_fd, &marker_name) {
        Ok(marker) => marker,
        Err(error) if file_lookup_is_missing(&error) => return Ok(RemoveExactFileOutcome::Missing),
        Err(error) => return Err(error),
    };
    if !file_contents_equal(&mut file, expected_contents)?
        || !file_contents_equal(&mut marker, expected_marker_contents)?
    {
        return Ok(RemoveExactFileOutcome::ContentsMismatch);
    }

    // The marker is removed first so a target-name race fails closed with the shared file intact
    revalidate_file_identity(&parent_fd, &marker_name, &marker)?;
    unlinkat(&parent_fd, &marker_name, AtFlags::empty())?;
    revalidate_file_identity(&parent_fd, &file_name, &file)?;
    unlinkat(&parent_fd, &file_name, AtFlags::empty())?;
    sync_directory(&parent_fd)?;
    Ok(RemoveExactFileOutcome::Removed)
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

fn revalidate_file_identity(
    parent_fd: &OwnedFd,
    file_name: &OsString,
    file: &std::fs::File,
) -> io::Result<()> {
    let retained = fstat(file)?;
    let visible = statat(parent_fd, file_name, AtFlags::SYMLINK_NOFOLLOW)?;
    if retained.st_dev == visible.st_dev && retained.st_ino == visible.st_ino {
        return Ok(());
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        "regular file changed during guarded removal",
    ))
}

fn file_lookup_is_missing(error: &io::Error) -> bool {
    // Missing exact-pair members are idempotent while every other error fails closed
    error.kind() == io::ErrorKind::NotFound
}

#[cfg(test)]
#[path = "tests/remove.rs"]
mod tests;
