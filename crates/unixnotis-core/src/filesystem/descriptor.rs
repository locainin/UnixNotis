//! Stable directory anchors and descriptor-relative path traversal

use std::ffi::{OsStr, OsString};
use std::io;
use std::os::fd::OwnedFd;
use std::path::{Component, Path};

use rustix::fs::{fchmod, fsync, mkdirat, openat2, Mode, OFlags, ResolveFlags, CWD};

/// Outcome for the final component of recursive directory creation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreateDirectoryOutcome {
    /// The requested directory itself was created by this operation
    TargetCreated,
    /// The requested directory already existed when its retained descriptor was opened
    TargetAlreadyExisted,
}

pub(super) fn open_parent(path: &Path) -> io::Result<(OwnedFd, OsString)> {
    open_parent_with(path, MissingDirectory::Create(0o755))
}

pub(super) fn open_parent_existing(path: &Path) -> io::Result<(OwnedFd, OsString)> {
    open_parent_with(path, MissingDirectory::Reject)
}

pub(super) fn open_directory_for_creation(
    path: &Path,
    mode: u32,
) -> io::Result<(OwnedFd, CreateDirectoryOutcome)> {
    open_directory_path(path, MissingDirectory::Create(mode))
}

pub(super) fn open_target_directory(
    path: &Path,
) -> io::Result<Option<(OwnedFd, OsString, OwnedFd)>> {
    // Removal never creates missing parents as a side effect
    let (parent_fd, file_name) = match open_parent_existing(path) {
        Ok(parent) => parent,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    match open_directory_at(&parent_fd, &file_name) {
        Ok(directory_fd) => Ok(Some((parent_fd, file_name, directory_fd))),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

pub(super) fn sync_directory(directory_fd: &OwnedFd) -> io::Result<()> {
    Ok(fsync(directory_fd)?)
}

pub(super) const fn contained_resolve_flags() -> ResolveFlags {
    ResolveFlags::BENEATH
        .union(ResolveFlags::NO_SYMLINKS)
        .union(ResolveFlags::NO_MAGICLINKS)
}

pub(super) const fn anchor_resolve_flags() -> ResolveFlags {
    ResolveFlags::NO_SYMLINKS.union(ResolveFlags::NO_MAGICLINKS)
}

#[derive(Clone, Copy)]
enum MissingDirectory {
    Create(u32),
    Reject,
}

fn open_parent_with(
    path: &Path,
    missing_directory: MissingDirectory,
) -> io::Result<(OwnedFd, OsString)> {
    // Keeping the final name separate makes every later operation descriptor-relative
    let file_name = path
        .file_name()
        .filter(|name| !name.is_empty())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "target has no file name"))?
        .to_os_string();
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let (parent_fd, _created) = open_directory_path(parent, missing_directory)?;
    Ok((parent_fd, file_name))
}

fn open_directory_path(
    path: &Path,
    missing_directory: MissingDirectory,
) -> io::Result<(OwnedFd, CreateDirectoryOutcome)> {
    // Absolute and relative paths begin from different trusted anchors
    let mut directory_fd = open_anchor(path)?;
    let mut target_outcome = CreateDirectoryOutcome::TargetAlreadyExisted;

    for component in path.components() {
        match component {
            Component::Prefix(_) | Component::RootDir | Component::CurDir => {}
            Component::ParentDir => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "filesystem path cannot contain parent traversal",
                ));
            }
            Component::Normal(name) => {
                let (next_fd, component_created) =
                    open_directory_component(&directory_fd, name, missing_directory)?;
                directory_fd = next_fd;
                target_outcome = if component_created {
                    CreateDirectoryOutcome::TargetCreated
                } else {
                    CreateDirectoryOutcome::TargetAlreadyExisted
                };
            }
        }
    }

    Ok((directory_fd, target_outcome))
}

fn open_anchor(path: &Path) -> io::Result<OwnedFd> {
    openat2(
        CWD,
        if path.is_absolute() { "/" } else { "." },
        OFlags::DIRECTORY.union(OFlags::CLOEXEC),
        Mode::empty(),
        anchor_resolve_flags(),
    )
    .map_err(Into::into)
}

fn open_directory_component(
    parent_fd: &OwnedFd,
    name: &OsStr,
    missing_directory: MissingDirectory,
) -> io::Result<(OwnedFd, bool)> {
    match open_directory_at(parent_fd, name) {
        Ok(fd) => Ok((fd, false)),
        Err(error)
            if error.kind() == io::ErrorKind::NotFound
                && matches!(missing_directory, MissingDirectory::Create(_)) =>
        {
            let MissingDirectory::Create(mode) = missing_directory else {
                unreachable!("guard requires directory creation mode");
            };
            create_directory_component(parent_fd, name, mode)
        }
        Err(error) => Err(error),
    }
}

fn create_directory_component(
    parent_fd: &OwnedFd,
    name: &OsStr,
    mode: u32,
) -> io::Result<(OwnedFd, bool)> {
    let create_result = mkdirat(parent_fd, name, file_mode(mode)).map_err(Into::into);
    let created = classify_directory_creation(create_result)?;
    let directory_fd = open_directory_at(parent_fd, name)?;
    if created {
        // Exact permissions are restored because mkdir remains subject to the process umask
        fchmod(&directory_fd, file_mode(mode))?;
        fsync(&directory_fd)?;
        fsync(parent_fd)?;
    }
    Ok((directory_fd, created))
}

pub(super) fn classify_directory_creation(result: io::Result<()>) -> io::Result<bool> {
    match result {
        Ok(()) => Ok(true),
        Err(error) => match error.kind() {
            // A concurrent creator still passes the same no-follow open before use
            io::ErrorKind::AlreadyExists => Ok(false),
            _ => Err(error),
        },
    }
}

pub(super) fn open_directory_at(parent_fd: &OwnedFd, name: &OsStr) -> io::Result<OwnedFd> {
    openat2(
        parent_fd,
        name,
        OFlags::DIRECTORY
            .union(OFlags::CLOEXEC)
            .union(OFlags::NOFOLLOW),
        Mode::empty(),
        contained_resolve_flags(),
    )
    .map_err(Into::into)
}

const fn file_mode(mode: u32) -> Mode {
    Mode::from_raw_mode(mode & 0o777)
}

#[cfg(test)]
#[path = "tests/descriptor.rs"]
mod tests;
