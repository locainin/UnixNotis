//! Durable file replacement that does not follow target symlinks

use rustix::fs::{mkdirat, openat2, renameat, unlinkat, AtFlags, Mode, OFlags, ResolveFlags, CWD};
use std::ffi::OsString;
use std::fs;
use std::io::{self, Write};
use std::os::fd::OwnedFd;
use std::os::unix::fs::PermissionsExt;
use std::path::{Component, Path};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const TEMP_ATTEMPTS: u8 = 16;
static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Replace a regular file through an exclusive sibling temporary file
///
/// # Errors
///
/// Returns an error when containment checks fail or the temporary write, synchronization, target
/// validation, rename, or parent-directory synchronization cannot complete
pub fn write_file_atomic(path: &Path, contents: &[u8], mode: u32) -> io::Result<()> {
    publish_file_atomic(path, mode, |file| file.write_all(contents))
}

/// Replace a regular file while retaining its current permission bits
///
/// A missing destination receives `default_mode`. Existing special files and links are rejected
/// through the same descriptor-relative checks as [`write_file_atomic`]
///
/// # Errors
///
/// Returns an error when containment checks fail or the temporary write, synchronization, target
/// validation, rename, or parent-directory synchronization cannot complete
pub fn write_file_atomic_preserving_mode(
    path: &Path,
    contents: &[u8],
    default_mode: u32,
) -> io::Result<()> {
    let (parent_fd, file_name) = open_parent(path)?;
    let mode = existing_target_mode(&parent_fd, &file_name)?.unwrap_or(default_mode);
    write_file_atomic_at(parent_fd, &file_name, mode, |file| file.write_all(contents))
}

pub(super) fn publish_file_atomic(
    path: &Path,
    mode: u32,
    write_payload: impl FnOnce(&mut fs::File) -> io::Result<()>,
) -> io::Result<()> {
    let (parent_fd, file_name) = open_parent(path)?;
    validate_target(&parent_fd, &file_name)?;
    write_file_atomic_at(parent_fd, &file_name, mode, write_payload)
}

fn write_file_atomic_at(
    parent_fd: OwnedFd,
    file_name: &OsString,
    mode: u32,
    write_payload: impl FnOnce(&mut fs::File) -> io::Result<()>,
) -> io::Result<()> {
    let candidates = temp_candidates(file_name);
    let (temp_name, mut temp_file) = reserve_temp(&parent_fd, candidates, mode)?;

    if let Err(error) =
        write_payload(&mut temp_file).and_then(|()| set_mode_and_sync(&temp_file, mode))
    {
        drop(temp_file);
        let _ = unlinkat(&parent_fd, &temp_name, AtFlags::empty());
        return Err(error);
    }
    drop(temp_file);

    // A second check catches target swaps made while the payload was written
    if let Err(error) = validate_target(&parent_fd, file_name) {
        let _ = unlinkat(&parent_fd, &temp_name, AtFlags::empty());
        return Err(error);
    }
    if let Err(error) = renameat(&parent_fd, &temp_name, &parent_fd, file_name) {
        let _ = unlinkat(&parent_fd, &temp_name, AtFlags::empty());
        return Err(error.into());
    }
    sync_directory(parent_fd)
}

/// Create a new regular file without replacing any existing path
///
/// # Errors
///
/// Returns an error when the parent cannot be opened securely, the destination already has an
/// unsafe shape, or writing and synchronizing the new file fails
pub fn write_file_if_missing(path: &Path, contents: &[u8], mode: u32) -> io::Result<bool> {
    let (parent_fd, file_name) = open_parent(path)?;
    let fd = match openat2(
        &parent_fd,
        &file_name,
        OFlags::WRONLY
            .union(OFlags::CLOEXEC)
            .union(OFlags::CREATE)
            .union(OFlags::EXCL),
        file_mode(mode),
        contained_resolve_flags(),
    ) {
        Ok(fd) => fd,
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            // A collision is safe only when the existing destination is a regular file
            validate_existing_target(&parent_fd, &file_name)?;
            return Ok(false);
        }
        Err(error) => return Err(error.into()),
    };
    let mut file = fs::File::from(fd);
    if let Err(error) = file
        .write_all(contents)
        .and_then(|()| set_mode_and_sync(&file, mode))
    {
        drop(file);
        let _ = unlinkat(&parent_fd, &file_name, AtFlags::empty());
        return Err(error);
    }
    drop(file);
    sync_directory(parent_fd)?;
    Ok(true)
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

pub(super) fn open_regular_file(path: &Path) -> io::Result<fs::File> {
    let (parent_fd, file_name) = open_parent_existing(path)?;
    let fd = openat2(
        &parent_fd,
        &file_name,
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

fn open_parent(path: &Path) -> io::Result<(OwnedFd, OsString)> {
    open_parent_with(path, MissingParent::Create)
}

fn open_parent_existing(path: &Path) -> io::Result<(OwnedFd, OsString)> {
    open_parent_with(path, MissingParent::Reject)
}

#[derive(Clone, Copy)]
enum MissingParent {
    Create,
    Reject,
}

fn open_parent_with(path: &Path, missing_parent: MissingParent) -> io::Result<(OwnedFd, OsString)> {
    let file_name = path
        .file_name()
        .filter(|name| !name.is_empty())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "target has no file name"))?
        .to_os_string();
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut parent_fd = if path.is_absolute() {
        openat2(
            CWD,
            "/",
            OFlags::DIRECTORY.union(OFlags::CLOEXEC),
            Mode::empty(),
            anchor_resolve_flags(),
        )?
    } else {
        openat2(
            CWD,
            ".",
            OFlags::DIRECTORY.union(OFlags::CLOEXEC),
            Mode::empty(),
            anchor_resolve_flags(),
        )?
    };

    for component in parent.components() {
        match component {
            Component::Prefix(_) | Component::RootDir | Component::CurDir => {}
            Component::ParentDir => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "atomic write path cannot contain parent traversal",
                ));
            }
            Component::Normal(name) => {
                parent_fd = open_directory_component(&parent_fd, name, missing_parent)?;
            }
        }
    }
    Ok((parent_fd, file_name))
}

fn open_directory_component(
    parent_fd: &OwnedFd,
    name: &std::ffi::OsStr,
    missing_parent: MissingParent,
) -> io::Result<OwnedFd> {
    match openat2(
        parent_fd,
        name,
        OFlags::DIRECTORY.union(OFlags::CLOEXEC),
        Mode::empty(),
        contained_resolve_flags(),
    ) {
        Ok(fd) => Ok(fd),
        Err(error)
            if error.kind() == io::ErrorKind::NotFound
                && matches!(missing_parent, MissingParent::Create) =>
        {
            mkdirat(parent_fd, name, Mode::from_raw_mode(0o755))?;
            openat2(
                parent_fd,
                name,
                OFlags::DIRECTORY.union(OFlags::CLOEXEC),
                Mode::empty(),
                contained_resolve_flags(),
            )
            .map_err(Into::into)
        }
        Err(error) => Err(error.into()),
    }
}

fn validate_target(parent_fd: &OwnedFd, file_name: &OsString) -> io::Result<()> {
    existing_target_mode(parent_fd, file_name).map(|_mode| ())
}

fn existing_target_mode(parent_fd: &OwnedFd, file_name: &OsString) -> io::Result<Option<u32>> {
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
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn validate_existing_target(parent_fd: &OwnedFd, file_name: &OsString) -> io::Result<()> {
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

fn unsafe_target_error() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        "refusing to operate on a non-regular file target",
    )
}

fn temp_candidates(file_name: &OsString) -> impl Iterator<Item = OsString> + '_ {
    (0..TEMP_ATTEMPTS).map(move |attempt| {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos());
        let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let mut name = OsString::from(".");
        name.push(file_name);
        name.push(format!(
            ".{}.{nanos}.{counter}.{attempt}.tmp",
            std::process::id()
        ));
        name
    })
}

fn reserve_temp(
    parent_fd: &OwnedFd,
    candidates: impl IntoIterator<Item = OsString>,
    mode: u32,
) -> io::Result<(OsString, fs::File)> {
    for name in candidates {
        match openat2(
            parent_fd,
            &name,
            OFlags::WRONLY
                .union(OFlags::CLOEXEC)
                .union(OFlags::CREATE)
                .union(OFlags::EXCL),
            file_mode(mode),
            contained_resolve_flags(),
        ) {
            Ok(fd) => return Ok((name, fs::File::from(fd))),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "unable to reserve an exclusive temporary file",
    ))
}

fn set_mode_and_sync(file: &fs::File, mode: u32) -> io::Result<()> {
    // Mode is fixed before publication so readers never observe broad temporary permissions
    file.set_permissions(fs::Permissions::from_mode(mode & 0o777))?;
    file.sync_all()
}

fn sync_directory(parent_fd: OwnedFd) -> io::Result<()> {
    fs::File::from(parent_fd).sync_all()
}

const fn file_mode(mode: u32) -> Mode {
    Mode::from_raw_mode(mode & 0o777)
}

const fn contained_resolve_flags() -> ResolveFlags {
    ResolveFlags::BENEATH
        .union(ResolveFlags::NO_SYMLINKS)
        .union(ResolveFlags::NO_MAGICLINKS)
}

const fn anchor_resolve_flags() -> ResolveFlags {
    ResolveFlags::NO_SYMLINKS.union(ResolveFlags::NO_MAGICLINKS)
}

#[cfg(test)]
#[path = "../tests/filesystem/atomic.rs"]
mod tests;
