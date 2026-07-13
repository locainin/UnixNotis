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
    let (parent_fd, file_name) = open_parent(path)?;
    validate_target(&parent_fd, &file_name)?;
    let candidates = temp_candidates(&file_name);
    let (temp_name, mut temp_file) = reserve_temp(&parent_fd, candidates, mode)?;

    if let Err(error) = write_and_sync(&mut temp_file, contents, mode) {
        drop(temp_file);
        let _ = unlinkat(&parent_fd, &temp_name, AtFlags::empty());
        return Err(error);
    }
    drop(temp_file);

    // A second check catches target swaps made while the payload was written
    if let Err(error) = validate_target(&parent_fd, &file_name) {
        let _ = unlinkat(&parent_fd, &temp_name, AtFlags::empty());
        return Err(error);
    }
    if let Err(error) = renameat(&parent_fd, &temp_name, &parent_fd, &file_name) {
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
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => return Ok(false),
        Err(error) => return Err(error.into()),
    };
    let mut file = fs::File::from(fd);
    if let Err(error) = write_and_sync(&mut file, contents, mode) {
        drop(file);
        let _ = unlinkat(&parent_fd, &file_name, AtFlags::empty());
        return Err(error);
    }
    drop(file);
    sync_directory(parent_fd)?;
    Ok(true)
}

fn open_parent(path: &Path) -> io::Result<(OwnedFd, OsString)> {
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
            Component::Normal(name) => parent_fd = open_or_create_dir(&parent_fd, name)?,
        }
    }
    Ok((parent_fd, file_name))
}

fn open_or_create_dir(parent_fd: &OwnedFd, name: &std::ffi::OsStr) -> io::Result<OwnedFd> {
    match openat2(
        parent_fd,
        name,
        OFlags::DIRECTORY.union(OFlags::CLOEXEC),
        Mode::empty(),
        contained_resolve_flags(),
    ) {
        Ok(fd) => Ok(fd),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
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
    match openat2(
        parent_fd,
        file_name,
        OFlags::RDONLY
            .union(OFlags::CLOEXEC)
            .union(OFlags::NOFOLLOW),
        Mode::empty(),
        contained_resolve_flags(),
    ) {
        Ok(fd) => {
            if fs::File::from(fd).metadata()?.is_file() {
                Ok(())
            } else {
                Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "refusing to replace a non-file target",
                ))
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
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

fn write_and_sync(file: &mut fs::File, contents: &[u8], mode: u32) -> io::Result<()> {
    file.write_all(contents)?;
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
