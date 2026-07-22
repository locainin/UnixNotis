//! Preflighted recursive removal for regular-only directory trees

use std::ffi::{CStr, OsStr};
use std::io;
use std::os::fd::OwnedFd;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use rustix::fs::{fstat, statat, unlinkat, AtFlags, Dir, FileType};

use super::descriptor::{open_directory_at, open_target_directory, sync_directory};
use super::directory::{invalid_marker_error, validate_child_name};
use super::regular::{file_contents_equal, open_regular_file_at};

/// Recursively remove a directory containing only regular files and directories
///
/// Symbolic links and special files are rejected and left in place
///
/// # Errors
///
/// Returns an error when a path component or child has an unsafe shape, an entry changes during
/// traversal, or removal and synchronization cannot complete
pub fn remove_directory_tree(path: &Path) -> io::Result<bool> {
    let Some((parent_fd, file_name, directory_fd)) = open_target_directory(path)? else {
        return Ok(false);
    };
    remove_directory_contents(&directory_fd)?;
    drop(directory_fd);
    unlinkat(&parent_fd, &file_name, AtFlags::REMOVEDIR)?;
    sync_directory(&parent_fd)?;
    Ok(true)
}

/// Remove a marked regular-only directory tree through one retained root descriptor
///
/// The entire tree is checked before any entry is deleted. The ownership marker is read relative
/// to that same descriptor, and the visible root name must still identify it before final removal
///
/// # Errors
///
/// Returns an error when the path or marker is unsafe, marker bytes differ, the tree contains a
/// link or special file, an entry changes shape, or durable removal fails
pub fn remove_marked_directory_tree(
    path: &Path,
    marker_name: &OsStr,
    marker_contents: &[u8],
) -> io::Result<bool> {
    validate_child_name(marker_name)?;
    let Some((parent_fd, file_name, directory_fd)) = open_target_directory(path)? else {
        return Ok(false);
    };
    let marker_name = marker_name.to_os_string();
    let mut marker = open_regular_file_at(&directory_fd, &marker_name)
        .map_err(|_error| invalid_marker_error())?;
    if !file_contents_equal(&mut marker, marker_contents)? {
        return Err(invalid_marker_error());
    }

    // Preflight is intentionally read-only so one rejected child cannot cause partial deletion
    preflight_directory_contents(&directory_fd)?;
    remove_directory_contents(&directory_fd)?;
    revalidate_directory_identity(&parent_fd, &file_name, &directory_fd)?;
    drop(directory_fd);
    unlinkat(&parent_fd, &file_name, AtFlags::REMOVEDIR)?;
    sync_directory(&parent_fd)?;
    Ok(true)
}

fn remove_directory_contents(directory_fd: &OwnedFd) -> io::Result<()> {
    let mut entries = Dir::read_from(directory_fd)?;
    while let Some(entry) = entries.read() {
        let entry = entry?;
        let name = entry.file_name();
        if matches!(name.to_bytes(), b"." | b"..") {
            continue;
        }
        let stat = statat(directory_fd, name, AtFlags::SYMLINK_NOFOLLOW)?;
        let file_type = FileType::from_raw_mode(stat.st_mode);
        if file_type.is_file() {
            unlinkat(directory_fd, name, AtFlags::empty())?;
            sync_directory(directory_fd)?;
        } else if file_type.is_dir() {
            let child_fd = open_directory_at(directory_fd, OsStr::from_bytes(name.to_bytes()))?;
            remove_directory_contents(&child_fd)?;
            drop(child_fd);
            unlinkat(directory_fd, name, AtFlags::REMOVEDIR)?;
            sync_directory(directory_fd)?;
        } else {
            return Err(unsafe_tree_entry_error(name));
        }
    }
    Ok(())
}

pub(super) fn preflight_directory_contents(directory_fd: &OwnedFd) -> io::Result<()> {
    let mut entries = Dir::read_from(directory_fd)?;
    while let Some(entry) = entries.read() {
        let entry = entry?;
        let name = entry.file_name();
        if matches!(name.to_bytes(), b"." | b"..") {
            continue;
        }
        let stat = statat(directory_fd, name, AtFlags::SYMLINK_NOFOLLOW)?;
        let file_type = FileType::from_raw_mode(stat.st_mode);
        if file_type.is_file() {
            continue;
        }
        if file_type.is_dir() {
            let child_fd = open_directory_at(directory_fd, OsStr::from_bytes(name.to_bytes()))?;
            preflight_directory_contents(&child_fd)?;
            continue;
        }
        return Err(unsafe_tree_entry_error(name));
    }
    Ok(())
}

pub(super) fn revalidate_directory_identity(
    parent_fd: &OwnedFd,
    file_name: &OsStr,
    directory_fd: &OwnedFd,
) -> io::Result<()> {
    let retained = fstat(directory_fd)?;
    let visible = statat(parent_fd, file_name, AtFlags::SYMLINK_NOFOLLOW)?;
    if retained.st_dev == visible.st_dev
        && retained.st_ino == visible.st_ino
        && FileType::from_raw_mode(visible.st_mode).is_dir()
    {
        return Ok(());
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        "directory changed while guarded removal was in progress",
    ))
}

fn unsafe_tree_entry_error(name: &CStr) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        format!(
            "refusing unsafe entry inside directory tree: {}",
            name.to_string_lossy()
        ),
    )
}

#[cfg(test)]
#[path = "tests/tree.rs"]
mod tests;
