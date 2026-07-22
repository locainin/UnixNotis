//! Directory creation, ownership markers, and empty removal

use std::ffi::OsStr;
use std::io;
use std::path::{Component, Path};

use rustix::fs::{unlinkat, AtFlags};

use super::descriptor::{
    open_directory_for_creation, open_target_directory, sync_directory, CreateDirectoryOutcome,
};
use super::exact::{ensure_exact_file_at, EnsureExactFileOutcome};
use super::regular::{file_contents_equal, open_regular_file_at};

/// Create a directory and every missing parent without following links
///
/// Reports whether the final directory was created without conflating parent creation
///
/// # Errors
///
/// Returns an error when the path traverses upward or through a link, an existing component is not
/// a directory, or creation, permission repair, or synchronization fails
pub fn create_directory_all(path: &Path, mode: u32) -> io::Result<CreateDirectoryOutcome> {
    let (_directory_fd, outcome) = open_directory_for_creation(path, mode)?;
    Ok(outcome)
}

/// Create a directory with an ownership marker or validate the retained existing directory
///
/// Existing directories are never mutated until their marker bytes are proven through the same
/// directory descriptor used for the decision
///
/// # Errors
///
/// Returns an error for unsafe paths, invalid marker names, missing or mismatched ownership
/// markers, and directory or marker creation failures
pub fn ensure_marked_directory(
    path: &Path,
    directory_mode: u32,
    marker_name: &OsStr,
    marker_contents: &[u8],
    marker_mode: u32,
) -> io::Result<CreateDirectoryOutcome> {
    validate_child_name(marker_name)?;
    let (directory_fd, outcome) = open_directory_for_creation(path, directory_mode)?;
    let marker_name = marker_name.to_os_string();

    match outcome {
        CreateDirectoryOutcome::TargetCreated => {
            let marker_outcome =
                ensure_exact_file_at(&directory_fd, &marker_name, marker_contents, marker_mode)?;
            if matches!(marker_outcome, EnsureExactFileOutcome::ContentsMismatch) {
                return Err(invalid_marker_error());
            }
        }
        CreateDirectoryOutcome::TargetAlreadyExisted => {
            let mut marker = open_regular_file_at(&directory_fd, &marker_name)
                .map_err(|_error| invalid_marker_error())?;
            if !file_contents_equal(&mut marker, marker_contents)? {
                return Err(invalid_marker_error());
            }
        }
    }

    Ok(outcome)
}

/// Remove an empty directory without following links
///
/// # Errors
///
/// Returns an error when a path component is unsafe, the target is not an empty directory, or the
/// removal or parent-directory synchronization fails
pub fn remove_empty_directory(path: &Path) -> io::Result<bool> {
    let Some((parent_fd, file_name, directory_fd)) = open_target_directory(path)? else {
        return Ok(false);
    };
    drop(directory_fd);
    unlinkat(&parent_fd, &file_name, AtFlags::REMOVEDIR)?;
    sync_directory(&parent_fd)?;
    Ok(true)
}

pub(super) fn validate_child_name(name: &OsStr) -> io::Result<()> {
    let mut components = Path::new(name).components();
    if matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none() {
        return Ok(());
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        "ownership marker must be one relative file name",
    ))
}

pub(super) fn invalid_marker_error() -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        "directory ownership marker is missing or does not match",
    )
}

#[cfg(test)]
#[path = "tests/directory.rs"]
mod tests;
