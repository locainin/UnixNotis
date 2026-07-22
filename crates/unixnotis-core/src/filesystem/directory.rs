//! Directory traversal, creation, and removal through stable descriptors

use std::ffi::{OsStr, OsString};
use std::io;
use std::os::fd::OwnedFd;
use std::os::unix::ffi::OsStrExt;
use std::path::{Component, Path};

use rustix::fs::{
    fchmod, fstat, fsync, mkdirat, openat2, statat, unlinkat, AtFlags, Dir, FileType, Mode, OFlags,
    ResolveFlags, CWD,
};

use super::exact::{ensure_exact_file_at, EnsureExactFileOutcome};
use super::regular::{file_contents_equal, open_regular_file_at};

/// Outcome for the final component of recursive directory creation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreateDirectoryOutcome {
    /// The requested directory itself was created by this operation
    TargetCreated,
    /// The requested directory already existed when its retained descriptor was opened
    TargetAlreadyExisted,
}

/// Create a directory and every missing parent without following links
///
/// Reports whether the final directory was created without conflating parent creation
///
/// # Errors
///
/// Returns an error when the path traverses upward or through a link, an existing component is not
/// a directory, or creation, permission repair, or synchronization fails
pub fn create_directory_all(path: &Path, mode: u32) -> io::Result<CreateDirectoryOutcome> {
    let (_directory_fd, outcome) = open_directory_path(path, MissingDirectory::Create(mode))?;
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
    let (directory_fd, outcome) =
        open_directory_path(path, MissingDirectory::Create(directory_mode))?;
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

/// Remove a marked regular-only directory tree through one retained root descriptor
///
/// The entire tree is checked before any entry is deleted. The ownership marker is read relative
/// to that same descriptor, and the visible root name must still identify it before final removal
///
/// # Errors
///
/// Returns an error when the path or marker is unsafe, marker bytes differ, the tree contains a
/// link or special file, an entry changes shape, or durable removal fails
pub fn remove_marked_directory_tree(
    path: &Path,
    marker_name: &OsStr,
    marker_contents: &[u8],
) -> io::Result<bool> {
    validate_child_name(marker_name)?;
    let Some((parent_fd, file_name, directory_fd)) = open_target_directory(path)? else {
        return Ok(false);
    };
    let marker_name = marker_name.to_os_string();
    let mut marker = open_regular_file_at(&directory_fd, &marker_name)
        .map_err(|_error| invalid_marker_error())?;
    if !file_contents_equal(&mut marker, marker_contents)? {
        return Err(invalid_marker_error());
    }

    // Preflight is intentionally read-only so one rejected child cannot cause partial deletion
    preflight_directory_contents(&directory_fd)?;
    remove_directory_contents(&directory_fd)?;
    revalidate_directory_identity(&parent_fd, &file_name, &directory_fd)?;
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
) -> io::Result<(OwnedFd, CreateDirectoryOutcome)> {
    // Absolute and relative paths begin from different trusted anchors
    let mut directory_fd = open_anchor(path)?;
    let mut target_outcome = CreateDirectoryOutcome::TargetAlreadyExisted;

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
    let create_result = mkdirat(parent_fd, name, file_mode(mode)).map_err(Into::into);
    let created = classify_directory_creation(create_result)?;
    let directory_fd = open_directory_at(parent_fd, name)?;
    if created {
        // Apply the exact requested mode because mkdir remains subject to the process umask
        fchmod(&directory_fd, file_mode(mode))?;
        fsync(&directory_fd)?;
        fsync(parent_fd)?;
    }
    Ok((directory_fd, created))
}

fn classify_directory_creation(result: io::Result<()>) -> io::Result<bool> {
    match result {
        Ok(()) => Ok(true),
        // A concurrent creator still has to pass the same no-follow directory open below
        Err(error) => match error.kind() {
            io::ErrorKind::AlreadyExists => Ok(false),
            _ => Err(error),
        },
    }
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

fn preflight_directory_contents(directory_fd: &OwnedFd) -> io::Result<()> {
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
            continue;
        }
        if file_type.is_dir() {
            let child_fd = open_directory_at(directory_fd, OsStr::from_bytes(name.to_bytes()))?;
            preflight_directory_contents(&child_fd)?;
            continue;
        }
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "refusing unsafe entry inside directory tree: {}",
                name.to_string_lossy()
            ),
        ));
    }
    Ok(())
}

fn revalidate_directory_identity(
    parent_fd: &OwnedFd,
    file_name: &OsStr,
    directory_fd: &OwnedFd,
) -> io::Result<()> {
    let retained = fstat(directory_fd)?;
    let visible = statat(parent_fd, file_name, AtFlags::SYMLINK_NOFOLLOW)?;
    if retained.st_dev == visible.st_dev
        && retained.st_ino == visible.st_ino
        && FileType::from_raw_mode(visible.st_mode).is_dir()
    {
        return Ok(());
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        "directory changed while guarded removal was in progress",
    ))
}

fn validate_child_name(name: &OsStr) -> io::Result<()> {
    let mut components = Path::new(name).components();
    if matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none() {
        return Ok(());
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        "ownership marker must be one relative file name",
    ))
}

fn invalid_marker_error() -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        "directory ownership marker is missing or does not match",
    )
}

const fn file_mode(mode: u32) -> Mode {
    Mode::from_raw_mode(mode & 0o777)
}

#[cfg(test)]
#[path = "tests/directory.rs"]
mod tests;
