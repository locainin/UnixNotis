//! Stable-descriptor operations for regular files

use std::ffi::OsString;
use std::fs;
use std::io::{self, Read};
use std::os::fd::OwnedFd;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use rustix::fs::{openat2, Mode, OFlags};

use super::descriptor::{contained_resolve_flags, open_parent_existing};

/// Open one regular file through a no-follow descriptor path
///
/// # Errors
///
/// Returns an error when any path component is a link, the target is not a regular file, or the
/// descriptor-relative open fails
pub fn open_regular_file(path: &Path) -> io::Result<fs::File> {
    let (parent_fd, file_name) = open_parent_existing(path)?;
    open_regular_file_at(&parent_fd, &file_name)
}

/// Compare one regular file with expected bytes through a single retained descriptor
///
/// Files larger than `maximum_size` are reported as non-matching without being read
///
/// # Errors
///
/// Returns an error when the expected bytes exceed the declared limit, the path is unsafe, the
/// target is not a regular file, or the bounded comparison cannot complete
pub fn regular_file_contents_equal(
    path: &Path,
    expected: &[u8],
    maximum_size: u64,
) -> io::Result<bool> {
    let expected_size = u64::try_from(expected.len()).map_err(|_error| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "expected regular-file contents do not fit the size limit",
        )
    })?;
    if expected_size > maximum_size {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "expected regular-file contents exceed the size limit",
        ));
    }

    // One open pins the object used by both the size check and byte comparison
    let mut file = open_regular_file(path)?;
    if file.metadata()?.len() > maximum_size {
        return Ok(false);
    }
    file_contents_equal(&mut file, expected)
}

/// Read a regular file without following links and enforce a byte limit
///
/// # Errors
///
/// Returns an error when the path crosses a link, the target is not a regular file, the file is
/// larger than `max_bytes`, or the bounded read cannot complete
pub fn read_regular_file_bounded(path: &Path, max_bytes: u64) -> io::Result<Vec<u8>> {
    // Opening once keeps the size check and payload read tied to one filesystem object
    let mut file = open_regular_file(path)?;
    let initial_size = file.metadata()?.len();
    if initial_size > max_bytes {
        return Err(limit_error(max_bytes));
    }

    let capacity = usize::try_from(initial_size).map_err(|_size_error| {
        io::Error::new(io::ErrorKind::InvalidData, "file size does not fit memory")
    })?;
    let mut contents = Vec::with_capacity(capacity);
    file.by_ref()
        .take(max_bytes.saturating_add(1))
        .read_to_end(&mut contents)?;
    if u64::try_from(contents.len()).unwrap_or(u64::MAX) > max_bytes {
        return Err(limit_error(max_bytes));
    }
    Ok(contents)
}

/// Add executable bits to an existing regular file without following links
///
/// # Errors
///
/// Returns an error when the path escapes through a link, is not a regular file, or cannot be
/// opened and updated through its stable descriptor
pub fn make_file_executable(path: &Path) -> io::Result<()> {
    let file = open_regular_file(path)?;
    let mode = file.metadata()?.permissions().mode() | 0o111;
    file.set_permissions(fs::Permissions::from_mode(mode))
}

/// Set permission bits on an existing regular file without following links
///
/// # Errors
///
/// Returns an error when the path escapes through a link, is not a regular file, or cannot be
/// opened and updated through its stable descriptor
pub fn set_file_mode(path: &Path, mode: u32) -> io::Result<()> {
    let file = open_regular_file(path)?;
    file.set_permissions(fs::Permissions::from_mode(mode & 0o777))
}

pub(super) fn open_regular_file_at(
    parent_fd: &OwnedFd,
    file_name: &OsString,
) -> io::Result<fs::File> {
    let fd = openat2(
        parent_fd,
        file_name,
        OFlags::RDONLY
            .union(OFlags::NONBLOCK)
            .union(OFlags::CLOEXEC)
            .union(OFlags::NOFOLLOW),
        Mode::empty(),
        contained_resolve_flags(),
    )?;
    let file = fs::File::from(fd);
    if !file.metadata()?.is_file() {
        return Err(unsafe_target_error());
    }
    Ok(file)
}

pub(super) fn file_contents_equal(file: &mut fs::File, expected: &[u8]) -> io::Result<bool> {
    let read_limit = u64::try_from(expected.len())
        .unwrap_or(u64::MAX)
        .saturating_add(1);
    let mut actual = Vec::with_capacity(expected.len().saturating_add(1));
    file.take(read_limit).read_to_end(&mut actual)?;
    Ok(actual == expected)
}

pub(super) fn existing_target_mode(
    parent_fd: &OwnedFd,
    file_name: &OsString,
) -> io::Result<Option<u32>> {
    match openat2(
        parent_fd,
        file_name,
        OFlags::PATH.union(OFlags::CLOEXEC).union(OFlags::NOFOLLOW),
        Mode::empty(),
        contained_resolve_flags(),
    ) {
        Ok(fd) => {
            let metadata = fs::File::from(fd).metadata()?;
            if metadata.is_file() {
                Ok(Some(metadata.permissions().mode() & 0o777))
            } else {
                Err(unsafe_target_error())
            }
        }
        Err(error) => match error.kind() {
            io::ErrorKind::NotFound => Ok(None),
            _ => Err(error.into()),
        },
    }
}

pub(super) fn validate_existing_target(
    parent_fd: &OwnedFd,
    file_name: &OsString,
) -> io::Result<()> {
    let fd = openat2(
        parent_fd,
        file_name,
        OFlags::PATH.union(OFlags::CLOEXEC).union(OFlags::NOFOLLOW),
        Mode::empty(),
        contained_resolve_flags(),
    )?;
    if fs::File::from(fd).metadata()?.is_file() {
        Ok(())
    } else {
        Err(unsafe_target_error())
    }
}

pub(super) fn unsafe_target_error() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        "refusing to operate on a non-regular file target",
    )
}

fn limit_error(max_bytes: u64) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("regular file exceeds the {max_bytes}-byte limit"),
    )
}

#[cfg(test)]
#[path = "tests/regular.rs"]
mod tests;
