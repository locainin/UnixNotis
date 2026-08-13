//! Symbolic-link inspection and publication through stable parent descriptors

use std::ffi::{OsStr, OsString};
use std::io;
use std::os::fd::OwnedFd;
use std::os::unix::ffi::OsStringExt;
use std::path::{Path, PathBuf};

use rustix::fs::{
    fstat, openat2, readlinkat, renameat, statat, symlinkat, unlinkat, AtFlags, FileType, Mode,
    OFlags,
};

use super::atomic::temp_candidates;
use super::descriptor::{
    contained_resolve_flags, open_parent, open_parent_existing, sync_directory,
};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SymlinkCreateAttempt {
    Created,
    Collision,
}

/// Create a symbolic link while preserving every existing path
///
/// # Errors
///
/// Returns an error when a parent crosses a link, the destination is an existing non-link, or link
/// creation and parent-directory synchronization fail
pub fn create_symlink_if_missing(path: &Path, target: &Path) -> io::Result<CreateSymlinkOutcome> {
    // Parent creation and lookup stay beneath one no-follow directory walk
    let (parent_fd, file_name) = open_parent(path)?;
    match read_symlink_at(&parent_fd, &file_name) {
        // Exact links are idempotent and avoid a new directory entry
        Ok(existing) => return Ok(existing_link_outcome(existing, target)),
        Err(error) => match error.kind() {
            io::ErrorKind::NotFound => {}
            _ => return Err(error),
        },
    }

    let create_result = symlinkat(target, &parent_fd, &file_name).map_err(Into::into);
    match classify_symlink_creation(create_result)? {
        SymlinkCreateAttempt::Created => {
            sync_directory(&parent_fd)?;
            Ok(CreateSymlinkOutcome::Created)
        }
        SymlinkCreateAttempt::Collision => {
            // A concurrent creator is accepted only when it published the requested link
            let existing = read_symlink_at(&parent_fd, &file_name)?;
            Ok(existing_link_outcome(existing, target))
        }
    }
}

fn classify_symlink_creation(result: io::Result<()>) -> io::Result<SymlinkCreateAttempt> {
    match result {
        Ok(()) => Ok(SymlinkCreateAttempt::Created),
        Err(error) => match error.kind() {
            io::ErrorKind::AlreadyExists => Ok(SymlinkCreateAttempt::Collision),
            _ => Err(error),
        },
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
        Ok(existing) => match existing_link_outcome(existing, target) {
            CreateSymlinkOutcome::Unchanged => return Ok(false),
            CreateSymlinkOutcome::TargetMismatch(_) => {}
            CreateSymlinkOutcome::Created => unreachable!("existing links cannot be newly created"),
        },
        Err(error) => match error.kind() {
            io::ErrorKind::NotFound => {}
            _ => return Err(error),
        },
    }

    // The replacement is prepared under an exclusive sibling name
    let temp_name = reserve_temp_symlink(&parent_fd, temp_candidates(&file_name), target)?;
    // Revalidation prevents known non-link targets from being overwritten
    if let Err(error) = validate_symlink_or_missing(&parent_fd, &file_name) {
        let _ = unlinkat(&parent_fd, &temp_name, AtFlags::empty());
        return Err(error);
    }
    // One rename publishes the complete link without an absent-target window
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
    // Inspection never creates a missing parent directory
    let (parent_fd, file_name) = match open_parent_existing(path) {
        Ok(parent) => parent,
        Err(error) => match error.kind() {
            io::ErrorKind::NotFound => return Ok(None),
            _ => return Err(error),
        },
    };
    match read_symlink_at(&parent_fd, &file_name) {
        Ok(target) => Ok(Some(target)),
        Err(error) => match error.kind() {
            io::ErrorKind::NotFound => Ok(None),
            _ => Err(error),
        },
    }
}

pub(super) fn read_symlink_at(parent_fd: &OwnedFd, file_name: &OsStr) -> io::Result<PathBuf> {
    let target = readlinkat(parent_fd, file_name, Vec::new())?;
    Ok(PathBuf::from(OsString::from_vec(target.into_bytes())))
}

pub(super) fn open_symlink_at(parent_fd: &OwnedFd, file_name: &OsString) -> io::Result<OwnedFd> {
    // O_PATH plus NOFOLLOW retains the link itself instead of opening its target
    let fd = openat2(
        parent_fd,
        file_name,
        OFlags::PATH.union(OFlags::CLOEXEC).union(OFlags::NOFOLLOW),
        Mode::empty(),
        contained_resolve_flags(),
    )?;
    let stat = fstat(&fd)?;
    if FileType::from_raw_mode(stat.st_mode).is_symlink() {
        return Ok(fd);
    }
    Err(not_symlink_error())
}

pub(super) fn revalidate_symlink_identity(
    parent_fd: &OwnedFd,
    file_name: &OsString,
    link: &OwnedFd,
) -> io::Result<()> {
    // Compare the retained link object with the visible basename immediately before unlinking
    let retained = fstat(link)?;
    let visible = statat(parent_fd, file_name, AtFlags::SYMLINK_NOFOLLOW)?;
    if retained.st_dev == visible.st_dev
        && retained.st_ino == visible.st_ino
        && FileType::from_raw_mode(visible.st_mode).is_symlink()
    {
        return Ok(());
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        "symbolic link changed before removal",
    ))
}

fn not_symlink_error() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        "refusing to operate on a non-symbolic-link target",
    )
}

fn reserve_temp_symlink(
    parent_fd: &OwnedFd,
    candidates: impl IntoIterator<Item = OsString>,
    target: &Path,
) -> io::Result<OsString> {
    // Exclusive candidates make planted temporary names harmless collisions
    for temp_name in candidates {
        match symlinkat(target, parent_fd, &temp_name) {
            Ok(()) => return Ok(temp_name),
            Err(error) => match error.kind() {
                io::ErrorKind::AlreadyExists => continue,
                _ => return Err(error.into()),
            },
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "unable to reserve an exclusive temporary symbolic link",
    ))
}

fn validate_symlink_or_missing(parent_fd: &OwnedFd, file_name: &OsStr) -> io::Result<()> {
    // Regular files, directories, and special objects fail through readlinkat
    match read_symlink_at(parent_fd, file_name) {
        Ok(_target) => Ok(()),
        Err(error) => match error.kind() {
            io::ErrorKind::NotFound => Ok(()),
            _ => Err(error),
        },
    }
}

fn existing_link_outcome(existing: PathBuf, target: &Path) -> CreateSymlinkOutcome {
    if existing == target {
        CreateSymlinkOutcome::Unchanged
    } else {
        CreateSymlinkOutcome::TargetMismatch(existing)
    }
}

#[cfg(test)]
#[path = "tests/symlink.rs"]
mod tests;
