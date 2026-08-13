//! Create-or-validate transactions for exact regular-file state

use std::ffi::OsString;
use std::fs;
use std::io::{self, Write};
use std::os::fd::OwnedFd;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use rustix::fs::{fstat, openat2, statat, unlinkat, AtFlags, Mode, OFlags};

use super::descriptor::{contained_resolve_flags, open_parent, sync_directory};
use super::regular::{file_contents_equal, open_regular_file_at};

/// Result of creating or validating a file whose bytes must match exactly
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnsureExactFileOutcome {
    /// The destination was absent and this operation created it
    Created,
    /// The existing regular file already contained the required bytes
    AlreadyExact,
    /// The existing regular file belongs to another owner or configuration
    ContentsMismatch,
}

/// Result of creating or validating an exact file-and-marker pair
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnsureExactFilePairOutcome {
    /// At least one missing member was created and the complete pair is exact
    Created,
    /// Both existing regular files already contained the required bytes
    AlreadyExact,
    /// The primary file was already exact but no ownership marker existed
    AlreadyExactUnowned,
    /// At least one existing member contained different bytes
    ContentsMismatch,
}

struct ExactMember {
    file: fs::File,
    created: bool,
}

#[derive(Clone, Copy)]
struct ExactMode(u32);

impl ExactMode {
    const fn new(mode: u32) -> Self {
        Self(mode & 0o777)
    }

    const fn rustix(self) -> Mode {
        Mode::from_raw_mode(self.0)
    }

    const fn permissions(self) -> u32 {
        self.0
    }
}

enum ExactMemberResult {
    Exact(ExactMember),
    ContentsMismatch,
}

/// Create a regular file when absent or validate an exact existing payload
///
/// A collision is opened once through the retained parent descriptor and is never replaced
///
/// # Errors
///
/// Returns an error when the parent path is unsafe, the destination is not a regular file, or
/// creating, reading, applying the mode, or synchronizing the file fails
pub fn ensure_exact_file(
    path: &Path,
    contents: &[u8],
    mode: u32,
) -> io::Result<EnsureExactFileOutcome> {
    let (parent_fd, file_name) = open_parent(path)?;
    ensure_exact_file_at(&parent_fd, &file_name, contents, mode)
}

/// Create or validate a same-directory regular file and ownership marker as one transaction
///
/// Existing members are never replaced. When either member conflicts, files created by this
/// operation are removed through retained descriptors before returning
///
/// # Errors
///
/// Returns an error when the paths do not share one parent, a path is unsafe, either target is not
/// a regular file, or creation, rollback, permission repair, or synchronization fails
pub fn ensure_exact_file_pair(
    path: &Path,
    contents: &[u8],
    mode: u32,
    marker_path: &Path,
    marker_contents: &[u8],
    marker_mode: u32,
) -> io::Result<EnsureExactFilePairOutcome> {
    if path.parent() != marker_path.parent() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "exact file pair must share one parent directory",
        ));
    }

    let (parent_fd, file_name) = open_parent(path)?;
    let marker_name = marker_path
        .file_name()
        .filter(|name| !name.is_empty())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "marker has no file name"))?
        .to_os_string();
    if file_name == marker_name {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "exact file pair must use two distinct names",
        ));
    }

    let mode = ExactMode::new(mode);
    let marker_mode = ExactMode::new(marker_mode);
    let file = match create_or_validate_member(&parent_fd, &file_name, contents, mode)? {
        ExactMemberResult::Exact(member) => member,
        ExactMemberResult::ContentsMismatch => {
            return Ok(EnsureExactFilePairOutcome::ContentsMismatch);
        }
    };
    let marker = if file.created {
        match create_or_validate_member(&parent_fd, &marker_name, marker_contents, marker_mode) {
            Ok(ExactMemberResult::Exact(member)) => member,
            Ok(ExactMemberResult::ContentsMismatch) => {
                rollback_created_member(&parent_fd, &file_name, &file)?;
                sync_directory(&parent_fd)?;
                return Ok(EnsureExactFilePairOutcome::ContentsMismatch);
            }
            Err(error) => {
                return Err(rollback_after_error(&parent_fd, &file_name, &file, error));
            }
        }
    } else {
        // An existing unmarked file may be compatible user state, so never claim it retroactively
        match open_regular_file_at(&parent_fd, &marker_name) {
            Ok(mut marker_file) => {
                if !file_contents_equal(&mut marker_file, marker_contents)? {
                    return Ok(EnsureExactFilePairOutcome::ContentsMismatch);
                }
                ExactMember {
                    file: marker_file,
                    created: false,
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(EnsureExactFilePairOutcome::AlreadyExactUnowned);
            }
            Err(error) => return Err(error),
        }
    };

    // Modes are repaired only after both retained payloads prove the complete pair is owned
    if let Err(error) = set_mode_and_sync(&file.file, mode)
        .and_then(|()| set_mode_and_sync(&marker.file, marker_mode))
        .and_then(|()| sync_directory(&parent_fd))
    {
        return Err(rollback_pair_after_error(
            &parent_fd,
            (&file_name, &file),
            (&marker_name, &marker),
            error,
        ));
    }

    if file.created || marker.created {
        Ok(EnsureExactFilePairOutcome::Created)
    } else {
        Ok(EnsureExactFilePairOutcome::AlreadyExact)
    }
}

pub(super) fn ensure_exact_file_at(
    parent_fd: &OwnedFd,
    file_name: &OsString,
    contents: &[u8],
    mode: u32,
) -> io::Result<EnsureExactFileOutcome> {
    let mode = ExactMode::new(mode);
    let member = match create_or_validate_member(parent_fd, file_name, contents, mode)? {
        ExactMemberResult::Exact(member) => member,
        ExactMemberResult::ContentsMismatch => {
            return Ok(EnsureExactFileOutcome::ContentsMismatch);
        }
    };

    if let Err(error) = set_mode_and_sync(&member.file, mode) {
        return Err(rollback_after_error(parent_fd, file_name, &member, error));
    }
    if let Err(error) = sync_directory(parent_fd) {
        return Err(rollback_after_error(parent_fd, file_name, &member, error));
    }

    if member.created {
        Ok(EnsureExactFileOutcome::Created)
    } else {
        Ok(EnsureExactFileOutcome::AlreadyExact)
    }
}

fn create_or_validate_member(
    parent_fd: &OwnedFd,
    file_name: &OsString,
    contents: &[u8],
    mode: ExactMode,
) -> io::Result<ExactMemberResult> {
    let fd = match openat2(
        parent_fd,
        file_name,
        OFlags::RDWR
            .union(OFlags::NONBLOCK)
            .union(OFlags::CLOEXEC)
            .union(OFlags::CREATE)
            .union(OFlags::EXCL),
        mode.rustix(),
        contained_resolve_flags(),
    ) {
        Ok(fd) => fd,
        Err(rustix::io::Errno::EXIST) => {
            let mut file = open_regular_file_at(parent_fd, file_name)?;
            if !file_contents_equal(&mut file, contents)? {
                return Ok(ExactMemberResult::ContentsMismatch);
            }
            return Ok(ExactMemberResult::Exact(ExactMember {
                file,
                created: false,
            }));
        }
        Err(error) => return Err(error.into()),
    };

    let mut file = fs::File::from(fd);
    if let Err(error) = file
        .write_all(contents)
        .and_then(|()| set_mode_and_sync(&file, mode))
    {
        let member = ExactMember {
            file,
            created: true,
        };
        return Err(rollback_after_error(parent_fd, file_name, &member, error));
    }
    Ok(ExactMemberResult::Exact(ExactMember {
        file,
        created: true,
    }))
}

fn rollback_pair_after_error(
    parent_fd: &OwnedFd,
    file: (&OsString, &ExactMember),
    marker: (&OsString, &ExactMember),
    error: io::Error,
) -> io::Error {
    let marker_rollback = rollback_created_member(parent_fd, marker.0, marker.1);
    let file_rollback = rollback_created_member(parent_fd, file.0, file.1);
    let directory_sync = sync_directory(parent_fd);
    combine_rollback_error(
        error,
        marker_rollback.and(file_rollback).and(directory_sync),
    )
}

fn rollback_after_error(
    parent_fd: &OwnedFd,
    file_name: &OsString,
    member: &ExactMember,
    error: io::Error,
) -> io::Error {
    let rollback = rollback_created_member(parent_fd, file_name, member)
        .and_then(|()| sync_directory(parent_fd));
    combine_rollback_error(error, rollback)
}

fn combine_rollback_error(error: io::Error, rollback: io::Result<()>) -> io::Error {
    match rollback {
        Ok(()) => error,
        Err(rollback_error) => io::Error::new(
            rollback_error.kind(),
            format!("{error}; exact-file rollback also failed: {rollback_error}"),
        ),
    }
}

fn rollback_created_member(
    parent_fd: &OwnedFd,
    file_name: &OsString,
    member: &ExactMember,
) -> io::Result<()> {
    if !member.created {
        return Ok(());
    }

    // Identity revalidation prevents rollback from removing a replacement object
    let retained = fstat(&member.file)?;
    let visible = match statat(parent_fd, file_name, AtFlags::SYMLINK_NOFOLLOW) {
        Ok(visible) => visible,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    if retained.st_dev != visible.st_dev || retained.st_ino != visible.st_ino {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "created exact file changed before rollback",
        ));
    }
    unlinkat(parent_fd, file_name, AtFlags::empty())?;
    Ok(())
}

fn set_mode_and_sync(file: &fs::File, mode: ExactMode) -> io::Result<()> {
    file.set_permissions(fs::Permissions::from_mode(mode.permissions()))?;
    file.sync_all()
}

#[cfg(test)]
#[path = "tests/exact.rs"]
mod tests;
