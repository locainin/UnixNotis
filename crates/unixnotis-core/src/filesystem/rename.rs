//! No-replace regular-file moves through stable parent descriptors

use std::io;
use std::path::Path;

use rustix::fs::{renameat_with, RenameFlags};

use super::descriptor::{open_parent_existing, open_target_directory, sync_directory};
use super::regular::validate_existing_target;
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
    // Final-component validation rejects source links, directories, and special files
    match validate_existing_target(&source_parent, &source_name) {
        Ok(()) => {}
        Err(error) => match error.kind() {
            io::ErrorKind::NotFound => return Ok(RenameRegularFileOutcome::SourceMissing),
            _ => return Err(error),
        },
    }

    let (destination_parent, destination_name) = open_parent_existing(destination)?;
    // Kernel no-replace semantics close the check-then-rename destination race
    let rename_result = renameat_with(
        &source_parent,
        &source_name,
        &destination_parent,
        &destination_name,
        RenameFlags::NOREPLACE,
    )
    .map_err(Into::into);
    match classify_rename_attempt(rename_result)? {
        RenameRegularFileOutcome::Renamed => {}
        outcome => return Ok(outcome),
    }

    // Both directory entries must reach durable storage even when parents differ
    sync_directory(&destination_parent)?;
    sync_directory(&source_parent)?;
    Ok(RenameRegularFileOutcome::Renamed)
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
    // The retained descriptor ensures the visible source name still identifies the staged tree
    revalidate_directory_identity(&source_parent, &source_name, &source_directory)?;

    let rename_result = renameat_with(
        &source_parent,
        &source_name,
        &destination_parent,
        &destination_name,
        RenameFlags::NOREPLACE,
    )
    .map_err(Into::into);
    let outcome = classify_directory_rename_attempt(rename_result)?;
    if outcome != RenameDirectoryOutcome::Renamed {
        return Ok(outcome);
    }

    sync_directory(&destination_parent)?;
    sync_directory(&source_parent)?;
    Ok(RenameDirectoryOutcome::Renamed)
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
