//! Directory traversal, creation, and removal through stable descriptors

use std::ffi::{OsStr, OsString};
use std::io;
use std::os::fd::OwnedFd;
use std::os::unix::ffi::OsStrExt;
use std::path::{Component, Path};

use rustix::fs::{
    fchmod, fsync, mkdirat, openat2, statat, unlinkat, AtFlags, Dir, FileType, Mode, OFlags,
    ResolveFlags, CWD,
};

/// Create a directory and every missing parent without following links
///
/// Returns `true` when at least the requested directory had to be created
///
/// # Errors
///
/// Returns an error when the path traverses upward or through a link, an existing component is not
/// a directory, or creation, permission repair, or synchronization fails
pub fn create_directory_all(path: &Path, mode: u32) -> io::Result<bool> {
    let (_directory_fd, created) = open_directory_path(path, MissingDirectory::Create(mode))?;
    Ok(created)
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

/// Recursively remove a directory containing only regular files and directories
///
/// Symbolic links and special files are rejected and left in place
///
/// # Errors
///
/// Returns an error when a path component or child has an unsafe shape, an entry changes during
/// traversal, or removal and synchronization cannot complete
pub fn remove_directory_tree(path: &Path) -> io::Result<bool> {
    let Some((parent_fd, file_name, directory_fd)) = open_target_directory(path)? else {
        return Ok(false);
    };
    remove_directory_contents(&directory_fd)?;
    drop(directory_fd);
    unlinkat(&parent_fd, &file_name, AtFlags::REMOVEDIR)?;
    sync_directory(&parent_fd)?;
    Ok(true)
}

pub(super) fn open_parent(path: &Path) -> io::Result<(OwnedFd, OsString)> {
    open_parent_with(path, MissingDirectory::Create(0o755))
}

pub(super) fn open_parent_existing(path: &Path) -> io::Result<(OwnedFd, OsString)> {
    open_parent_with(path, MissingDirectory::Reject)
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
) -> io::Result<(OwnedFd, bool)> {
    // Absolute and relative paths begin from different trusted anchors
    let mut directory_fd = open_anchor(path)?;
    let mut created = false;

    for component in path.components() {
        match component {
            // Anchors already account for root and current-directory components
            Component::Prefix(_) | Component::RootDir | Component::CurDir => {}
            Component::ParentDir => {
                // Upward traversal would break the beneath policy of the current descriptor
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "filesystem path cannot contain parent traversal",
                ));
            }
            Component::Normal(name) => {
                let (next_fd, component_created) =
                    open_directory_component(&directory_fd, name, missing_directory)?;
                directory_fd = next_fd;
                created |= component_created;
            }
        }
    }

    Ok((directory_fd, created))
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
            // Creation is attempted only after a no-follow open proves the component absent
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
    let created = match mkdirat(parent_fd, name, file_mode(mode)) {
        Ok(()) => true,
        // A concurrent creator still has to pass the same no-follow directory open below
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => false,
        Err(error) => return Err(error.into()),
    };
    let directory_fd = open_directory_at(parent_fd, name)?;
    if created {
        // Apply the exact requested mode because mkdir remains subject to the process umask
        fchmod(&directory_fd, file_mode(mode))?;
        fsync(&directory_fd)?;
        fsync(parent_fd)?;
    }
    Ok((directory_fd, created))
}

fn open_directory_at(parent_fd: &OwnedFd, name: &OsStr) -> io::Result<OwnedFd> {
    // NOFOLLOW covers the final component while resolve flags cover every nested lookup
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

fn open_target_directory(path: &Path) -> io::Result<Option<(OwnedFd, OsString, OwnedFd)>> {
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

fn remove_directory_contents(directory_fd: &OwnedFd) -> io::Result<()> {
    // Dir reads from the retained descriptor even if the visible pathname changes later
    let mut entries = Dir::read_from(directory_fd)?;
    while let Some(entry) = entries.read() {
        let entry = entry?;
        let name = entry.file_name();
        if matches!(name.to_bytes(), b"." | b"..") {
            continue;
        }
        let stat = statat(directory_fd, name, AtFlags::SYMLINK_NOFOLLOW)?;
        let file_type = FileType::from_raw_mode(stat.st_mode);
        if file_type.is_file() {
            // Regular children can be unlinked without opening their contents
            unlinkat(directory_fd, name, AtFlags::empty())?;
            fsync(directory_fd)?;
        } else if file_type.is_dir() {
            // Child recursion receives another no-follow descriptor before deleting anything
            let child_fd = open_directory_at(directory_fd, OsStr::from_bytes(name.to_bytes()))?;
            remove_directory_contents(&child_fd)?;
            drop(child_fd);
            unlinkat(directory_fd, name, AtFlags::REMOVEDIR)?;
            fsync(directory_fd)?;
        } else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "refusing unsafe entry inside directory tree: {}",
                    name.to_string_lossy()
                ),
            ));
        }
    }
    Ok(())
}

const fn file_mode(mode: u32) -> Mode {
    Mode::from_raw_mode(mode & 0o777)
}

#[cfg(test)]
#[path = "../tests/filesystem/directory.rs"]
mod tests;
