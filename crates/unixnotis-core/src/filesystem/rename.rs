//! No-replace regular-file moves through stable parent descriptors

use std::io;
use std::path::Path;

use rustix::fs::{renameat_with, RenameFlags};

use super::descriptor::{open_parent_existing, open_target_directory, sync_directory};
use super::quarantine::{Quarantine, QuarantinedEntry};
use super::regular::{open_regular_file_at, revalidate_file_identity};
use super::tree::revalidate_directory_identity;

/// Result of moving a regular file without replacing another filesystem entry
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenameRegularFileOutcome {
    /// The source did not exist when the move reached the filesystem boundary
    SourceMissing,
    /// The source was moved to the previously unused destination
    Renamed,
    /// A destination entry already existed and was preserved
    DestinationExists,
}

/// Result of moving a directory without replacing another filesystem entry
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenameDirectoryOutcome {
    /// The source did not exist when the move reached the filesystem boundary
    SourceMissing,
    /// The source was moved to the previously unused destination
    Renamed,
    /// A destination entry already existed and was preserved
    DestinationExists,
}

/// Move a regular file without following links or replacing the destination
///
/// # Errors
///
/// Returns an error when either parent crosses a link, the source is not a regular file, or the
/// rename and directory synchronization cannot complete
pub fn rename_regular_file_no_replace(
    source: &Path,
    destination: &Path,
) -> io::Result<RenameRegularFileOutcome> {
    let (source_parent, source_name) = match open_parent_existing(source) {
        Ok(parent) => parent,
        Err(error) => match error.kind() {
            io::ErrorKind::NotFound => return Ok(RenameRegularFileOutcome::SourceMissing),
            _ => return Err(error),
        },
    };
    // Retain the validated source so a replacement basename cannot be claimed
    let source_file = match open_regular_file_at(&source_parent, &source_name) {
        Ok(file) => file,
        Err(error) => match error.kind() {
            io::ErrorKind::NotFound => return Ok(RenameRegularFileOutcome::SourceMissing),
            _ => return Err(error),
        },
    };

    let (destination_parent, destination_name) = open_parent_existing(destination)?;
    // Claim the source basename before publication so no later operation uses a watched source
    // name to identify the object being moved
    let quarantine = Quarantine::create(&source_parent)?;
    let entry = match quarantine.move_entry(&source_parent, &source_name) {
        Ok(entry) => entry,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let _ = quarantine.cleanup(&source_parent);
            return Ok(RenameRegularFileOutcome::SourceMissing);
        }
        Err(error) => {
            let _ = quarantine.cleanup(&source_parent);
            return Err(error);
        }
    };
    if let Err(error) = revalidate_file_identity(quarantine.fd(), entry.name(), &source_file) {
        let error = restore_quarantined_entry_or_error(
            &quarantine,
            &entry,
            &source_parent,
            &source_name,
            error,
        );
        return finish_quarantine(quarantine, &source_parent, Err(error));
    }

    // Rename the quarantined entry itself so sparse data, metadata, ACLs, timestamps, and hard
    // links survive without copy-delete behavior
    // The quarantine descriptor pins the parent, while the entry name still relies on the private
    // directory boundary described by the quarantine module
    // The directory descriptor pins the quarantine parent; the final entry name remains a path
    // and therefore uses the same private-directory trust boundary
    let rename_result = renameat_with(
        quarantine.fd(),
        entry.name(),
        &destination_parent,
        &destination_name,
        RenameFlags::NOREPLACE,
    )
    .map_err(Into::into);
    let outcome = match classify_rename_attempt(rename_result) {
        Ok(outcome) => outcome,
        Err(error) => {
            let error = restore_quarantined_entry_or_error(
                &quarantine,
                &entry,
                &source_parent,
                &source_name,
                error,
            );
            return finish_quarantine(quarantine, &source_parent, Err(error));
        }
    };
    if outcome != RenameRegularFileOutcome::Renamed {
        let result = quarantine
            .restore(&entry, &source_parent, &source_name)
            .map(|()| outcome)
            .map_err(|error| {
                restore_quarantined_entry_or_error(
                    &quarantine,
                    &entry,
                    &source_parent,
                    &source_name,
                    error,
                )
            });
        return finish_quarantine(quarantine, &source_parent, result);
    }

    // Both final directory entries must reach durable storage
    let result = sync_directory(&destination_parent)
        .and(sync_directory(&source_parent))
        .map(|()| RenameRegularFileOutcome::Renamed);
    finish_quarantine(quarantine, &source_parent, result)
}

/// Move a directory without following links or replacing the destination
///
/// # Errors
///
/// Returns an error when either parent crosses a link, the source is not a directory, or the
/// rename and directory synchronization cannot complete
pub fn rename_directory_no_replace(
    source: &Path,
    destination: &Path,
) -> io::Result<RenameDirectoryOutcome> {
    let Some((source_parent, source_name, source_directory)) = open_target_directory(source)?
    else {
        return Ok(RenameDirectoryOutcome::SourceMissing);
    };
    let (destination_parent, destination_name) = open_parent_existing(destination)?;
    // Claim the staged directory basename before publication for the same reason as regular files
    let quarantine = Quarantine::create(&source_parent)?;
    let entry = match quarantine.move_entry(&source_parent, &source_name) {
        Ok(entry) => entry,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let _ = quarantine.cleanup(&source_parent);
            return Ok(RenameDirectoryOutcome::SourceMissing);
        }
        Err(error) => {
            let _ = quarantine.cleanup(&source_parent);
            return Err(error);
        }
    };
    if let Err(error) =
        revalidate_directory_identity(quarantine.fd(), entry.name(), &source_directory)
    {
        let error = restore_quarantined_entry_or_error(
            &quarantine,
            &entry,
            &source_parent,
            &source_name,
            error,
        );
        return finish_quarantine(quarantine, &source_parent, Err(error));
    }

    let rename_result = renameat_with(
        quarantine.fd(),
        entry.name(),
        &destination_parent,
        &destination_name,
        RenameFlags::NOREPLACE,
    )
    .map_err(Into::into);
    let outcome = match classify_directory_rename_attempt(rename_result) {
        Ok(outcome) => outcome,
        Err(error) => {
            let error = restore_quarantined_entry_or_error(
                &quarantine,
                &entry,
                &source_parent,
                &source_name,
                error,
            );
            return finish_quarantine(quarantine, &source_parent, Err(error));
        }
    };
    if outcome != RenameDirectoryOutcome::Renamed {
        let result = quarantine
            .restore(&entry, &source_parent, &source_name)
            .map(|()| outcome)
            .map_err(|error| {
                restore_quarantined_entry_or_error(
                    &quarantine,
                    &entry,
                    &source_parent,
                    &source_name,
                    error,
                )
            });
        return finish_quarantine(quarantine, &source_parent, result);
    }

    let result = sync_directory(&destination_parent)
        .and(sync_directory(&source_parent))
        .map(|()| RenameDirectoryOutcome::Renamed);
    finish_quarantine(quarantine, &source_parent, result)
}

fn restore_quarantined_entry_or_error(
    quarantine: &Quarantine,
    entry: &QuarantinedEntry,
    parent_fd: &std::os::fd::OwnedFd,
    claimed_name: &std::ffi::OsString,
    operation_error: io::Error,
) -> io::Error {
    match quarantine.restore(entry, parent_fd, claimed_name) {
        Ok(()) => operation_error,
        Err(restore_error) => io::Error::new(
            operation_error.kind(),
            format!("{operation_error}; failed to restore quarantine entry: {restore_error}"),
        ),
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
            Err(cleanup_error) => Err(io::Error::new(
                error.kind(),
                format!("{error}; failed to clean up quarantine: {cleanup_error}"),
            )),
        },
    }
}

fn classify_rename_attempt(result: io::Result<()>) -> io::Result<RenameRegularFileOutcome> {
    match result {
        Ok(()) => Ok(RenameRegularFileOutcome::Renamed),
        Err(error) => match error.kind() {
            io::ErrorKind::AlreadyExists => Ok(RenameRegularFileOutcome::DestinationExists),
            io::ErrorKind::NotFound => Ok(RenameRegularFileOutcome::SourceMissing),
            _ => Err(error),
        },
    }
}

fn classify_directory_rename_attempt(result: io::Result<()>) -> io::Result<RenameDirectoryOutcome> {
    match result {
        Ok(()) => Ok(RenameDirectoryOutcome::Renamed),
        Err(error) => match error.kind() {
            io::ErrorKind::AlreadyExists => Ok(RenameDirectoryOutcome::DestinationExists),
            io::ErrorKind::NotFound => Ok(RenameDirectoryOutcome::SourceMissing),
            _ => Err(error),
        },
    }
}

#[cfg(test)]
#[path = "tests/rename.rs"]
mod tests;
