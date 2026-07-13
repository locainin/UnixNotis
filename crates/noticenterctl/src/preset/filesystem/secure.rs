//! Secure directory-fd helpers for preset import writes on Linux
//!
//! These helpers keep the low-level openat2-based write path separate from the
//! generic filesystem helpers so the Linux-specific logic stays contained

use anyhow::{anyhow, Context, Result};
use chrono::Local;
use rustix::fs::{mkdirat, openat2, renameat, unlinkat, AtFlags, Mode, OFlags, ResolveFlags, CWD};
use std::fs;
use std::io::{Read, Write};
use std::os::fd::{AsFd, OwnedFd};
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use super::super::pathing::normalize_relative_path;

const BACKUP_PREFIX: &str = "Backup-";
const MAX_BACKUP_SUFFIX_ATTEMPTS: usize = 10_000;

pub fn open_secure_dir_all(path: &Path) -> Result<OwnedFd> {
    let mut current_fd = if path.is_absolute() {
        // Start from the real filesystem root so each later segment is opened under a stable dir fd
        openat2(
            CWD,
            "/",
            directory_open_flags(),
            Mode::empty(),
            secure_anchor_resolve_flags(),
        )
        .context("open filesystem root for secure directory walk")?
    } else {
        // Relative paths are anchored to the current shell directory
        openat2(
            CWD,
            ".",
            directory_open_flags(),
            Mode::empty(),
            secure_anchor_resolve_flags(),
        )
        .context("open current working directory for secure directory walk")?
    };

    for component in path.components() {
        match component {
            // These pieces only shape the anchor and do not open a child segment
            Component::Prefix(_) | Component::RootDir | Component::CurDir => {}
            Component::ParentDir => {
                return Err(anyhow!(
                    "secure directory walk does not allow parent traversal: {}",
                    path.display()
                ));
            }
            Component::Normal(part) => {
                // Each segment is opened under the previous dir fd so path reparsing stays contained
                current_fd = open_or_create_child_dir(&current_fd, part)?;
            }
        }
    }

    Ok(current_fd)
}

pub fn read_relative_file_secure(
    root_dir: &OwnedFd,
    relative_path: &Path,
) -> Result<(Vec<u8>, u32)> {
    // Normalize first so every later open call sees one clean relative path shape
    let relative_path = normalize_relative_path(relative_path)?;
    let file_fd = openat2(
        root_dir,
        &relative_path,
        read_open_flags(),
        Mode::empty(),
        secure_resolve_flags(),
    )
    .with_context(|| format!("open file under secure root {}", relative_path.display()))?;
    let mut file = fs::File::from(file_fd);
    let metadata = file
        .metadata()
        .with_context(|| format!("inspect file under secure root {}", relative_path.display()))?;
    if !metadata.is_file() {
        // Rollback and backup logic only make sense for regular files
        return Err(anyhow!(
            "secure file read expected a regular file under the UnixNotis config directory: {}",
            relative_path.display()
        ));
    }

    let mut contents = Vec::new();
    // Reads stay fully in-memory here because backup and rollback both reuse the original bytes
    file.read_to_end(&mut contents)
        .with_context(|| format!("read file under secure root {}", relative_path.display()))?;

    use std::os::unix::fs::PermissionsExt;
    Ok((contents, metadata.permissions().mode()))
}

pub fn write_relative_file_atomic_secure(
    root_dir: &OwnedFd,
    relative_path: &Path,
    contents: &[u8],
    mode: u32,
) -> Result<()> {
    // The secure parent walk happens first so the later temp file and rename stay beneath one root
    let relative_path = normalize_relative_path(relative_path)?;
    let (parent_fd, file_name) = open_or_create_parent_dir(root_dir, &relative_path)?;

    // Temp files stay in the final parent dir so the rename does not cross filesystems
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock is earlier than the Unix epoch")?
        .as_nanos();
    let temp_name = format!(".{file_name}.{stamp}.tmp");
    let temp_fd = openat2(
        &parent_fd,
        temp_name.as_str(),
        temp_file_open_flags(),
        mode_from_bits(mode),
        secure_resolve_flags(),
    )
    .with_context(|| format!("create secure temp file for {}", relative_path.display()))?;
    let mut temp_file = fs::File::from(temp_fd);

    let write_result = (|| -> Result<()> {
        // The payload stays hidden in the temp file until every write and flush step is done
        temp_file
            .write_all(contents)
            .with_context(|| format!("write secure temp file for {}", relative_path.display()))?;
        temp_file
            .sync_all()
            .with_context(|| format!("flush secure temp file for {}", relative_path.display()))?;
        Ok(())
    })();

    if let Err(err) = write_result {
        // Failed temp writes should not leave junk beside the final target path
        let _ = unlinkat(&parent_fd, temp_name.as_str(), AtFlags::empty());
        return Err(err);
    }

    // Rename is the only point where the new contents become visible at the target path
    renameat(
        &parent_fd,
        temp_name.as_str(),
        &parent_fd,
        file_name.as_str(),
    )
    .with_context(|| format!("replace secure target file {}", relative_path.display()))?;
    Ok(())
}

pub fn remove_relative_file_secure(root_dir: &OwnedFd, relative_path: &Path) -> Result<()> {
    // Deletes reuse the same secure parent lookup so a raced path cannot redirect the unlink
    let relative_path = normalize_relative_path(relative_path)?;
    let (parent_fd, file_name) = open_or_create_parent_dir(root_dir, &relative_path)?;
    unlinkat(&parent_fd, file_name.as_str(), AtFlags::empty())
        .with_context(|| format!("remove secure target file {}", relative_path.display()))?;
    Ok(())
}

pub fn create_backup_dir_secure(root_dir: &OwnedFd) -> Result<(PathBuf, OwnedFd)> {
    // Backup names match the rest of the project, but creation stays pinned to the secure root fd
    let stamp = Local::now().format("%Y-%m-%d").to_string();

    // A finite cap prevents a corrupted directory from causing an endless retry loop
    for suffix in 0..MAX_BACKUP_SUFFIX_ATTEMPTS {
        let dir_name = if suffix == 0 {
            format!("{BACKUP_PREFIX}{stamp}")
        } else {
            format!("{BACKUP_PREFIX}{stamp}-{suffix:03}")
        };

        match mkdirat(root_dir, dir_name.as_str(), mode_from_bits(0o755)) {
            Ok(()) => {
                // Returning the opened dir fd lets the caller keep writing without path reparsing
                let dir_fd = openat2(
                    root_dir,
                    dir_name.as_str(),
                    directory_open_flags(),
                    Mode::empty(),
                    secure_resolve_flags(),
                )
                .with_context(|| format!("open secure backup directory {dir_name}"))?;
                return Ok((PathBuf::from(dir_name), dir_fd));
            }
            Err(err) if backup_name_is_taken(err.kind()) => {
                // Existing backup names are skipped so repeated imports keep every earlier snapshot
                continue;
            }
            Err(err) => {
                return Err(err)
                    .with_context(|| format!("create secure backup directory {dir_name}"))
            }
        }
    }

    Err(anyhow!("exhausted every available backup directory suffix"))
}

pub fn remove_empty_relative_dirs_secure(root_dir: &OwnedFd, relative_path: &Path) -> Result<()> {
    // Cleanup walks upward from the deepest parent until one directory is still needed
    let relative_path = normalize_relative_path(relative_path)?;
    let mut current: Option<PathBuf> = relative_path.parent().map(Path::to_path_buf);

    while let Some(path) = current {
        if path.as_os_str().is_empty() {
            break;
        }

        match unlinkat(root_dir, &path, AtFlags::REMOVEDIR) {
            Ok(()) => {
                // Keep walking upward until a parent still contains something else
                current = path.parent().map(PathBuf::from);
            }
            Err(err) if empty_dir_cleanup_is_complete(err.kind()) => {
                // Missing or non-empty directories mean cleanup is already done far enough
                break;
            }
            Err(err) => {
                return Err(err)
                    .with_context(|| format!("remove secure directory {}", path.display()))
            }
        }
    }

    Ok(())
}

pub fn remove_relative_dir_secure(root_dir: &OwnedFd, relative_path: &Path) -> Result<()> {
    // Snapshot-root cleanup happens last, after every nested file has already been removed
    let relative_path = normalize_relative_path(relative_path)?;
    unlinkat(root_dir, &relative_path, AtFlags::REMOVEDIR)
        .with_context(|| format!("remove secure directory {}", relative_path.display()))?;
    Ok(())
}

fn open_or_create_parent_dir(
    root_dir: &OwnedFd,
    relative_path: &Path,
) -> Result<(OwnedFd, String)> {
    // The final file name is split out once so the parent walk only deals with directory segments
    let file_name = relative_path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .ok_or_else(|| {
            anyhow!(
                "relative file path has no file name: {}",
                relative_path.display()
            )
        })?
        .to_string();

    let mut current_fd = openat2(
        root_dir,
        ".",
        directory_open_flags(),
        Mode::empty(),
        secure_resolve_flags(),
    )
    .context("open secure root directory handle")?;

    if let Some(parent) = relative_path.parent() {
        for component in parent.components() {
            match component {
                // "." does not change the secure parent walk
                Component::CurDir => {}
                Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                    return Err(anyhow!(
                        "secure parent walk does not allow this path component: {}",
                        relative_path.display()
                    ));
                }
                Component::Normal(part) => {
                    // Missing parent directories are created one segment at a time under the secure root
                    current_fd = open_or_create_child_dir(&current_fd, part)?;
                }
            }
        }
    }

    Ok((current_fd, file_name))
}

fn open_or_create_child_dir<Fd: AsFd>(parent_fd: Fd, name: &std::ffi::OsStr) -> Result<OwnedFd> {
    match openat2(
        parent_fd.as_fd(),
        name,
        directory_open_flags(),
        Mode::empty(),
        secure_resolve_flags(),
    ) {
        // Existing directories are reopened under the already trusted parent dir fd
        Ok(fd) => Ok(fd),
        Err(err) if child_directory_is_missing(err.kind()) => {
            // Create the next directory segment under the already verified parent dir fd
            mkdirat(parent_fd.as_fd(), name, mode_from_bits(0o755)).with_context(|| {
                format!("create secure directory {}", Path::new(name).display())
            })?;
            openat2(
                parent_fd.as_fd(),
                name,
                directory_open_flags(),
                Mode::empty(),
                secure_resolve_flags(),
            )
            .with_context(|| format!("open secure directory {}", Path::new(name).display()))
        }
        Err(err) => {
            // Other failures keep the segment name attached so the broken step is obvious
            Err(err).with_context(|| format!("open secure directory {}", Path::new(name).display()))
        }
    }
}

pub(super) const fn secure_resolve_flags() -> ResolveFlags {
    // BENEATH pins all later lookups under the starting dir fd while the symlink bans stop jumps
    ResolveFlags::BENEATH
        .union(ResolveFlags::NO_SYMLINKS)
        .union(ResolveFlags::NO_MAGICLINKS)
}

pub(super) const fn secure_anchor_resolve_flags() -> ResolveFlags {
    // The anchor walk may start from / or . so it skips BENEATH and only bans link-like detours
    ResolveFlags::NO_SYMLINKS.union(ResolveFlags::NO_MAGICLINKS)
}

// Only a name collision is safe to retry with the next backup suffix
pub(super) fn backup_name_is_taken(kind: std::io::ErrorKind) -> bool {
    kind == std::io::ErrorKind::AlreadyExists
}

// Missing and non-empty parents both mean upward cleanup has reached its safe limit
pub(super) const fn empty_dir_cleanup_is_complete(kind: std::io::ErrorKind) -> bool {
    matches!(
        kind,
        std::io::ErrorKind::NotFound | std::io::ErrorKind::DirectoryNotEmpty
    )
}

// Directory creation is allowed only when the lookup proved the segment is absent
pub(super) fn child_directory_is_missing(kind: std::io::ErrorKind) -> bool {
    kind == std::io::ErrorKind::NotFound
}

pub(super) const fn directory_open_flags() -> OFlags {
    // Directory handles must never leak through a later process launch
    OFlags::DIRECTORY.union(OFlags::CLOEXEC)
}

pub(super) const fn read_open_flags() -> OFlags {
    // Secure reads need no write capability and must close across exec
    OFlags::RDONLY.union(OFlags::CLOEXEC)
}

pub(super) const fn temp_file_open_flags() -> OFlags {
    // Exclusive creation prevents a stale or attacker-planted temp file from being reused
    OFlags::WRONLY
        .union(OFlags::CLOEXEC)
        .union(OFlags::CREATE)
        .union(OFlags::EXCL)
}

const fn mode_from_bits(mode: u32) -> Mode {
    Mode::from_raw_mode(mode)
}
