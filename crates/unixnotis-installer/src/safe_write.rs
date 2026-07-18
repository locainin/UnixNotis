//! Symlink-aware file writes for user-owned config files

use rustix::fs::{mkdirat, openat2, renameat, unlinkat, AtFlags, Mode, OFlags, ResolveFlags, CWD};
use std::fs;
use std::io::{self, Write};
use std::os::fd::OwnedFd;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn write_text_preserving_mode(
    path: &Path,
    contents: &str,
    default_mode: u32,
) -> io::Result<()> {
    let mode = existing_mode_or_default(path, default_mode)?;
    write_text_with_mode(path, contents, mode)
}

pub fn write_text_with_mode(path: &Path, contents: &str, mode: u32) -> io::Result<()> {
    let (parent_fd, file_name) = open_secure_parent(path)?;
    validate_target_at(&parent_fd, &file_name)?;
    let (temp_name, mut temp_file) = create_atomic_temp_at(&parent_fd, &file_name, mode)?;
    let result = (|| -> io::Result<()> {
        temp_file.write_all(contents.as_bytes())?;
        temp_file.flush()?;
        #[cfg(unix)]
        {
            // Set the mode before rename so the visible file never appears too permissive
            temp_file.set_permissions(fs::Permissions::from_mode(mode & 0o777))?;
        }
        temp_file.sync_all()?;
        Ok(())
    })();

    if let Err(err) = result {
        let _ = unlinkat(&parent_fd, temp_name.as_str(), AtFlags::empty());
        return Err(err);
    }
    drop(temp_file);

    // Re-check immediately before rename so a late symlink swap is not silently followed
    validate_target_at(&parent_fd, &file_name).inspect_err(|_err| {
        let _ = unlinkat(&parent_fd, temp_name.as_str(), AtFlags::empty());
    })?;
    Ok(renameat(
        &parent_fd,
        temp_name.as_str(),
        &parent_fd,
        file_name.as_str(),
    )
    .inspect_err(|_err| {
        let _ = unlinkat(&parent_fd, temp_name.as_str(), AtFlags::empty());
    })?)
}

pub fn reject_unsafe_write_target(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("refusing to write through symlink {}", path.display()),
                ));
            }
            if metadata.is_file() {
                return Ok(());
            }
            Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("refusing to overwrite non-file {}", path.display()),
            ))
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err),
    }
}

fn existing_mode_or_default(path: &Path, default_mode: u32) -> io::Result<u32> {
    let (parent_fd, file_name) = open_secure_parent(path)?;
    match openat2(
        &parent_fd,
        file_name.as_str(),
        // O_PATH inspects metadata without opening FIFO or device contents
        OFlags::PATH.union(OFlags::CLOEXEC).union(OFlags::NOFOLLOW),
        Mode::empty(),
        secure_resolve_flags(),
    ) {
        Ok(fd) => {
            let file = fs::File::from(fd);
            let metadata = file.metadata()?;
            if !metadata.is_file() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("refusing to overwrite non-file {}", path.display()),
                ));
            }
            Ok(file_mode(&metadata))
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(default_mode),
        Err(err) => Err(err.into()),
    }
}

#[cfg(unix)]
fn file_mode(metadata: &fs::Metadata) -> u32 {
    metadata.permissions().mode() & 0o777
}

#[cfg(not(unix))]
fn file_mode(_metadata: &fs::Metadata) -> u32 {
    0o644
}

fn create_atomic_temp_at(
    parent_fd: &OwnedFd,
    file_name: &str,
    mode: u32,
) -> io::Result<(String, fs::File)> {
    for attempt in 0..16 {
        let temp_name = atomic_temp_name(file_name, attempt);
        match openat2(
            parent_fd,
            temp_name.as_str(),
            OFlags::WRONLY | OFlags::CLOEXEC | OFlags::CREATE | OFlags::EXCL,
            Mode::from_raw_mode(mode & 0o777),
            secure_resolve_flags(),
        ) {
            Ok(fd) => return Ok((temp_name, fs::File::from(fd))),
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {
                // Another installer run may have picked the same timestamp; retry with a new suffix
                continue;
            }
            Err(err) => return Err(err.into()),
        }
    }

    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "could not create secure temporary file",
    ))
}

fn atomic_temp_name(file_name: &str, attempt: u8) -> String {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock moved backwards")
        .as_nanos();
    format!(
        ".{file_name}.{}.{}.{}.tmp",
        std::process::id(),
        stamp,
        attempt
    )
}

fn open_secure_parent(path: &Path) -> io::Result<(OwnedFd, String)> {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "target file name is invalid"))?
        .to_string();
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "target path has no parent"))?;
    let mut current_fd = if path.is_absolute() {
        openat2(
            CWD,
            "/",
            OFlags::DIRECTORY.union(OFlags::CLOEXEC),
            Mode::empty(),
            ResolveFlags::empty(),
        )?
    } else {
        openat2(
            CWD,
            ".",
            OFlags::DIRECTORY.union(OFlags::CLOEXEC),
            Mode::empty(),
            ResolveFlags::empty(),
        )?
    };

    for component in parent.components() {
        match component {
            std::path::Component::Prefix(_)
            | std::path::Component::RootDir
            | std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "secure write path cannot contain parent traversal",
                ));
            }
            std::path::Component::Normal(part) => {
                current_fd = open_or_create_child_dir(&current_fd, part)?;
            }
        }
    }
    Ok((current_fd, file_name))
}

fn open_or_create_child_dir(parent_fd: &OwnedFd, name: &std::ffi::OsStr) -> io::Result<OwnedFd> {
    match openat2(
        parent_fd,
        name,
        OFlags::DIRECTORY.union(OFlags::CLOEXEC),
        Mode::empty(),
        secure_resolve_flags(),
    ) {
        Ok(fd) => Ok(fd),
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            mkdirat(parent_fd, name, Mode::from_raw_mode(0o755))?;
            Ok(openat2(
                parent_fd,
                name,
                OFlags::DIRECTORY.union(OFlags::CLOEXEC),
                Mode::empty(),
                secure_resolve_flags(),
            )?)
        }
        Err(err) => Err(err.into()),
    }
}

fn validate_target_at(parent_fd: &OwnedFd, file_name: &str) -> io::Result<()> {
    match openat2(
        parent_fd,
        file_name,
        OFlags::PATH.union(OFlags::CLOEXEC).union(OFlags::NOFOLLOW),
        Mode::empty(),
        secure_resolve_flags(),
    ) {
        Ok(fd) => {
            let metadata = fs::File::from(fd).metadata()?;
            if metadata.is_file() {
                Ok(())
            } else {
                Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "refusing to overwrite non-file target",
                ))
            }
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err.into()),
    }
}

const fn secure_resolve_flags() -> ResolveFlags {
    ResolveFlags::BENEATH
        .union(ResolveFlags::NO_SYMLINKS)
        .union(ResolveFlags::NO_MAGICLINKS)
}

#[cfg(test)]
#[path = "tests/safe_write.rs"]
mod tests;
