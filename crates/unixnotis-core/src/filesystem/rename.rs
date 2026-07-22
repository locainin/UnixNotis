//! No-replace regular-file moves through stable parent descriptors

use std::io;
use std::path::Path;

use rustix::fs::{renameat_with, RenameFlags};

use super::atomic::validate_existing_target;
use super::directory::{open_parent_existing, sync_directory};

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
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(RenameRegularFileOutcome::SourceMissing);
        }
        Err(error) => return Err(error),
    };
    // Final-component validation rejects source links, directories, and special files
    match validate_existing_target(&source_parent, &source_name) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(RenameRegularFileOutcome::SourceMissing);
        }
        Err(error) => return Err(error),
    }

    let (destination_parent, destination_name) = open_parent_existing(destination)?;
    // Kernel no-replace semantics close the check-then-rename destination race
    match renameat_with(
        &source_parent,
        &source_name,
        &destination_parent,
        &destination_name,
        RenameFlags::NOREPLACE,
    ) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            return Ok(RenameRegularFileOutcome::DestinationExists);
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(RenameRegularFileOutcome::SourceMissing);
        }
        Err(error) => return Err(error.into()),
    }

    // Both directory entries must reach durable storage even when parents differ
    sync_directory(&destination_parent)?;
    sync_directory(&source_parent)?;
    Ok(RenameRegularFileOutcome::Renamed)
}

#[cfg(test)]
#[path = "../tests/filesystem/rename.rs"]
mod tests;
