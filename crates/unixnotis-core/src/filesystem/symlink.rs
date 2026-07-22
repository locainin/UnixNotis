//! Symbolic-link inspection and publication through stable parent descriptors

use std::ffi::{OsStr, OsString};
use std::io;
use std::os::fd::OwnedFd;
use std::os::unix::ffi::OsStringExt;
use std::path::{Path, PathBuf};

use rustix::fs::{readlinkat, renameat, symlinkat, unlinkat, AtFlags};

use super::atomic::temp_candidates;
use super::directory::{open_parent, open_parent_existing, sync_directory};

/// Result of creating a symbolic link without replacing an existing path
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CreateSymlinkOutcome {
    /// A new link was created
    Created,
    /// The existing link already stored the requested target
    Unchanged,
    /// A different link target was preserved
    TargetMismatch(PathBuf),
}

/// Create a symbolic link while preserving every existing path
///
/// # Errors
///
/// Returns an error when a parent crosses a link, the destination is an existing non-link, or link
/// creation and parent-directory synchronization fail
pub fn create_symlink_if_missing(path: &Path, target: &Path) -> io::Result<CreateSymlinkOutcome> {
    let (parent_fd, file_name) = open_parent(path)?;
    match read_symlink_at(&parent_fd, &file_name) {
        Ok(existing) if existing == target => return Ok(CreateSymlinkOutcome::Unchanged),
        Ok(existing) => return Ok(CreateSymlinkOutcome::TargetMismatch(existing)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }

    match symlinkat(target, &parent_fd, &file_name) {
        Ok(()) => {
            sync_directory(&parent_fd)?;
            Ok(CreateSymlinkOutcome::Created)
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            // A concurrent creator is accepted only when it published the exact requested link
            match read_symlink_at(&parent_fd, &file_name)? {
                existing if existing == target => Ok(CreateSymlinkOutcome::Unchanged),
                existing => Ok(CreateSymlinkOutcome::TargetMismatch(existing)),
            }
        }
        Err(error) => Err(error.into()),
    }
}

/// Atomically create or replace a symbolic link
///
/// Existing non-link destinations are rejected. A matching existing link is left untouched
///
/// # Errors
///
/// Returns an error when a parent crosses a link, an existing destination is not a symbolic link,
/// or temporary-link creation, revalidation, rename, cleanup, or synchronization fails
pub fn replace_symlink_atomic(path: &Path, target: &Path) -> io::Result<bool> {
    let (parent_fd, file_name) = open_parent(path)?;
    match read_symlink_at(&parent_fd, &file_name) {
        Ok(existing) if existing == target => return Ok(false),
        Ok(_existing) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }

    let temp_name = reserve_temp_symlink(&parent_fd, &file_name, target)?;
    if let Err(error) = validate_symlink_or_missing(&parent_fd, &file_name) {
        let _ = unlinkat(&parent_fd, &temp_name, AtFlags::empty());
        return Err(error);
    }
    if let Err(error) = renameat(&parent_fd, &temp_name, &parent_fd, &file_name) {
        let _ = unlinkat(&parent_fd, &temp_name, AtFlags::empty());
        return Err(error.into());
    }
    sync_directory(&parent_fd)?;
    Ok(true)
}

/// Read a symbolic link target without following links in its parent path
///
/// # Errors
///
/// Returns an error when a parent crosses a link, the target is not a symbolic link, or the link
/// cannot be read
pub fn read_symlink(path: &Path) -> io::Result<Option<PathBuf>> {
    let (parent_fd, file_name) = match open_parent_existing(path) {
        Ok(parent) => parent,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    match read_symlink_at(&parent_fd, &file_name) {
        Ok(target) => Ok(Some(target)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

pub(super) fn read_symlink_at(parent_fd: &OwnedFd, file_name: &OsStr) -> io::Result<PathBuf> {
    let target = readlinkat(parent_fd, file_name, Vec::new())?;
    Ok(PathBuf::from(OsString::from_vec(target.into_bytes())))
}

fn reserve_temp_symlink(
    parent_fd: &OwnedFd,
    file_name: &OsString,
    target: &Path,
) -> io::Result<OsString> {
    for temp_name in temp_candidates(file_name) {
        match symlinkat(target, parent_fd, &temp_name) {
            Ok(()) => return Ok(temp_name),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "unable to reserve an exclusive temporary symbolic link",
    ))
}

fn validate_symlink_or_missing(parent_fd: &OwnedFd, file_name: &OsStr) -> io::Result<()> {
    match read_symlink_at(parent_fd, file_name) {
        Ok(_target) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
#[path = "../tests/filesystem/symlink.rs"]
mod tests;
