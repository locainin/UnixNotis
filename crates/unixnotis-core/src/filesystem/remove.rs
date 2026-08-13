//! Descriptor-relative removal through a private same-filesystem quarantine
//!
//! Each removal first claims the requested basename with an atomic rename. Validation then uses
//! the retained object descriptor inside the quarantine instead of reopening the visible path
//!
//! The final unlink remains pathname-based because Linux has no unlink-by-file-descriptor API.
//! The quarantine directory must therefore be inaccessible to hostile same-UID writers when the
//! caller needs protection beyond the normal other-UID filesystem boundary

use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};

use super::descriptor::open_parent_existing;
use super::quarantine::{Quarantine, QuarantinedEntry};
use super::regular::{file_contents_equal, open_regular_file_at, revalidate_file_identity};
use super::symlink::{open_symlink_at, read_symlink_at, revalidate_symlink_identity};

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
/// The source basename is first moved into a private mode-0700 directory on the same filesystem.
/// Identity checks and physical unlinking then use the retained quarantine directory descriptor
/// rather than a visible claim pathname
///
/// # Errors
///
/// Returns an error when a path component is unsafe, the target changes during quarantine, the
/// target is not a regular file, or quarantine cleanup cannot complete
pub fn remove_regular_file(path: &Path) -> io::Result<bool> {
    let Some((parent_fd, file_name)) = existing_parent(path)? else {
        return Ok(false);
    };
    let file = match open_regular_file_at(&parent_fd, &file_name) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    };
    let quarantine = Quarantine::create(&parent_fd)?;
    let entry = match quarantine.move_entry(&parent_fd, &file_name) {
        Ok(entry) => entry,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let _ = quarantine.cleanup(&parent_fd);
            return Ok(false);
        }
        Err(error) => {
            let _ = quarantine.cleanup(&parent_fd);
            return Err(error);
        }
    };

    let result =
        unlink_regular_entry(&quarantine, &entry, &parent_fd, &file_name, &file).map(|()| true);
    finish_quarantine(quarantine, &parent_fd, result)
}

/// Remove two same-directory regular files only when both retained payloads match
///
/// Both names are quarantined before either entry is physically unlinked. A mismatch leaves the
/// quarantined entry in place or restores it without deleting a replacement basename
///
/// # Errors
///
/// Returns an error when paths have different parents, path traversal is unsafe, either target is
/// not a regular file, retained identities change, or durable quarantine cleanup fails
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

    let quarantine = Quarantine::create(&parent_fd)?;
    let marker_entry = match quarantine.move_entry(&parent_fd, &marker_name) {
        Ok(entry) => entry,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let _ = quarantine.cleanup(&parent_fd);
            return Ok(RemoveExactFileOutcome::Missing);
        }
        Err(error) => {
            let _ = quarantine.cleanup(&parent_fd);
            return Err(error);
        }
    };
    if let Err(error) = revalidate_file_identity(quarantine.fd(), marker_entry.name(), &marker) {
        let error =
            restore_entry_or_error(&quarantine, &marker_entry, &parent_fd, &marker_name, error);
        return finish_quarantine(quarantine, &parent_fd, Err(error));
    }

    let file_entry = match quarantine.move_entry(&parent_fd, &file_name) {
        Ok(entry) => entry,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let result = match quarantine.restore(&marker_entry, &parent_fd, &marker_name) {
                Ok(()) => Ok(RemoveExactFileOutcome::Missing),
                Err(restore_error) => {
                    Err(combine_operation_and_restore_error(&error, &restore_error))
                }
            };
            return finish_quarantine(quarantine, &parent_fd, result);
        }
        Err(error) => {
            let error =
                restore_entry_or_error(&quarantine, &marker_entry, &parent_fd, &marker_name, error);
            return finish_quarantine(quarantine, &parent_fd, Err(error));
        }
    };
    if let Err(error) = revalidate_file_identity(quarantine.fd(), file_entry.name(), &file) {
        let error = restore_entry_or_error(&quarantine, &file_entry, &parent_fd, &file_name, error);
        let error =
            restore_entry_or_error(&quarantine, &marker_entry, &parent_fd, &marker_name, error);
        return finish_quarantine(quarantine, &parent_fd, Err(error));
    }

    // The marker is removed first to preserve the existing ownership protocol
    if let Err(error) = unlink_regular_entry(
        &quarantine,
        &marker_entry,
        &parent_fd,
        &marker_name,
        &marker,
    ) {
        let error = restore_entry_or_error(&quarantine, &file_entry, &parent_fd, &file_name, error);
        return finish_quarantine(quarantine, &parent_fd, Err(error));
    }
    let result = unlink_regular_entry(&quarantine, &file_entry, &parent_fd, &file_name, &file)
        .map(|()| RemoveExactFileOutcome::Removed);
    finish_quarantine(quarantine, &parent_fd, result)
}

/// Remove a symbolic link without requiring a specific target
///
/// # Errors
///
/// Returns an error when a path component is unsafe, the target is not a symbolic link, the
/// quarantined identity changes, or quarantine cleanup fails
pub fn remove_symlink(path: &Path) -> io::Result<bool> {
    let Some((parent_fd, file_name)) = existing_parent(path)? else {
        return Ok(false);
    };
    let link = match open_symlink_at(&parent_fd, &file_name) {
        Ok(link) => link,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    };
    let quarantine = Quarantine::create(&parent_fd)?;
    let entry = match quarantine.move_entry(&parent_fd, &file_name) {
        Ok(entry) => entry,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let _ = quarantine.cleanup(&parent_fd);
            return Ok(false);
        }
        Err(error) => {
            let _ = quarantine.cleanup(&parent_fd);
            return Err(error);
        }
    };
    let result =
        unlink_symlink_entry(&quarantine, &entry, &parent_fd, &file_name, &link).map(|()| true);
    finish_quarantine(quarantine, &parent_fd, result)
}

/// Remove a symbolic link only when its stored target matches exactly
///
/// # Errors
///
/// Returns an error when a path component is unsafe, the target is not a symbolic link, the
/// quarantined identity changes, or quarantine cleanup fails
pub fn remove_symlink_if_target(
    path: &Path,
    expected_target: &Path,
) -> io::Result<RemoveSymlinkOutcome> {
    let Some((parent_fd, file_name)) = existing_parent(path)? else {
        return Ok(RemoveSymlinkOutcome::Missing);
    };
    let link = match open_symlink_at(&parent_fd, &file_name) {
        Ok(link) => link,
        Err(error) => match error.kind() {
            io::ErrorKind::NotFound => return Ok(RemoveSymlinkOutcome::Missing),
            _ => return Err(error),
        },
    };
    let actual_target = match read_symlink_at(&parent_fd, &file_name) {
        Ok(target) => target,
        Err(error) => match error.kind() {
            io::ErrorKind::NotFound => return Ok(RemoveSymlinkOutcome::Missing),
            _ => return Err(error),
        },
    };
    if actual_target != expected_target {
        return Ok(RemoveSymlinkOutcome::TargetMismatch(actual_target));
    }

    let quarantine = Quarantine::create(&parent_fd)?;
    let entry = match quarantine.move_entry(&parent_fd, &file_name) {
        Ok(entry) => entry,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let _ = quarantine.cleanup(&parent_fd);
            return Ok(RemoveSymlinkOutcome::Missing);
        }
        Err(error) => {
            let _ = quarantine.cleanup(&parent_fd);
            return Err(error);
        }
    };
    let quarantined_target = match read_symlink_at(quarantine.fd(), entry.name()) {
        Ok(target) => target,
        Err(error) => {
            let error = restore_entry_or_error(&quarantine, &entry, &parent_fd, &file_name, error);
            return finish_quarantine(quarantine, &parent_fd, Err(error));
        }
    };
    if quarantined_target != expected_target {
        let mismatch_error = io::Error::new(
            io::ErrorKind::InvalidInput,
            "symbolic-link target changed during quarantine",
        );
        let result = match quarantine.restore(&entry, &parent_fd, &file_name) {
            Ok(()) => Ok(RemoveSymlinkOutcome::TargetMismatch(quarantined_target)),
            Err(restore_error) => Err(combine_operation_and_restore_error(
                &mismatch_error,
                &restore_error,
            )),
        };
        return finish_quarantine(quarantine, &parent_fd, result);
    }

    let result = unlink_symlink_entry(&quarantine, &entry, &parent_fd, &file_name, &link)
        .map(|()| RemoveSymlinkOutcome::Removed);
    finish_quarantine(quarantine, &parent_fd, result)
}

fn unlink_regular_entry(
    quarantine: &Quarantine,
    entry: &QuarantinedEntry,
    source_parent: &std::os::fd::OwnedFd,
    source_name: &OsString,
    file: &std::fs::File,
) -> io::Result<()> {
    // Revalidate the object after the atomic claim so a failed claim never authorizes a new file
    revalidate_file_identity(quarantine.fd(), entry.name(), file).map_err(|error| {
        restore_entry_or_error(quarantine, entry, source_parent, source_name, error)
    })?;
    // The retained quarantine descriptor pins the parent; the final basename is still resolved
    // by unlinkat, so a hostile writer must not control this private directory
    quarantine.unlink(entry).map_err(|error| {
        restore_entry_or_error(quarantine, entry, source_parent, source_name, error)
    })
}

fn unlink_symlink_entry(
    quarantine: &Quarantine,
    entry: &QuarantinedEntry,
    source_parent: &std::os::fd::OwnedFd,
    source_name: &OsString,
    link: &std::os::fd::OwnedFd,
) -> io::Result<()> {
    // Symlink identity is checked without following the stored target
    revalidate_symlink_identity(quarantine.fd(), entry.name(), link).map_err(|error| {
        restore_entry_or_error(quarantine, entry, source_parent, source_name, error)
    })?;
    // As with regular files, unlinkat protects the parent directory but not a hostile same-UID
    // replacement of the final quarantine basename between validation and the syscall
    quarantine.unlink(entry).map_err(|error| {
        restore_entry_or_error(quarantine, entry, source_parent, source_name, error)
    })
}

fn restore_entry_or_error(
    quarantine: &Quarantine,
    entry: &QuarantinedEntry,
    source_parent: &std::os::fd::OwnedFd,
    source_name: &OsString,
    operation_error: io::Error,
) -> io::Error {
    match quarantine.restore(entry, source_parent, source_name) {
        Ok(()) => operation_error,
        Err(restore_error) => combine_operation_and_restore_error(&operation_error, &restore_error),
    }
}

fn finish_quarantine<T>(
    quarantine: Quarantine,
    parent_fd: &std::os::fd::OwnedFd,
    result: io::Result<T>,
) -> io::Result<T> {
    let cleanup = quarantine.cleanup(parent_fd);
    match result {
        Ok(value) => {
            cleanup?;
            Ok(value)
        }
        Err(error) => match cleanup {
            Ok(()) => Err(error),
            Err(cleanup_error) => Err(combine_operation_and_restore_error(&error, &cleanup_error)),
        },
    }
}

fn combine_operation_and_restore_error(
    operation_error: &io::Error,
    restore_error: &io::Error,
) -> io::Error {
    io::Error::new(
        operation_error.kind(),
        format!("{operation_error}; failed to restore quarantine entry: {restore_error}"),
    )
}

fn existing_parent(path: &Path) -> io::Result<Option<(std::os::fd::OwnedFd, OsString)>> {
    // The optional form keeps idempotent removal separate from unsafe-shape failures
    match open_parent_existing(path) {
        Ok(parent) => Ok(Some(parent)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

fn file_lookup_is_missing(error: &io::Error) -> bool {
    // Missing exact-pair members are idempotent while every other error fails closed
    error.kind() == io::ErrorKind::NotFound
}

#[cfg(test)]
#[path = "tests/remove.rs"]
mod tests;
