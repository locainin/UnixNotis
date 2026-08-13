//! Private same-filesystem quarantine directories for exact entry retirement
//!
//! The first rename is the security boundary for the visible source name. It moves the entry
//! out of the watched basename in one kernel operation, so a later replacement cannot be
//! mistaken for the original source entry
//!
//! The retained quarantine descriptor pins the directory used by later checks and operations.
//! It does not turn the final entry name into a file-descriptor operation: Linux unlinkat still
//! resolves the final entry by pathname. Mode 0700 therefore excludes other UIDs, while a
//! hostile process with the same UID still requires a trusted quarantine directory boundary

use std::ffi::OsString;
use std::fmt::Write as _;
use std::io;
use std::os::fd::OwnedFd;

use rustix::fs::{fchmod, mkdirat, renameat_with, unlinkat, AtFlags, Mode, RenameFlags};
use rustix::rand::{getrandom, GetRandomFlags};

use super::descriptor::{open_directory_at, sync_directory};

const QUARANTINE_ATTEMPTS: usize = 16;
const RANDOM_BYTES: usize = 16;
const QUARANTINE_PREFIX: &str = ".unixnotis-quarantine.";
const ENTRY_PREFIX: &str = ".unixnotis-entry.";

/// One private quarantine directory retained through a stable descriptor
pub(super) struct Quarantine {
    name: OsString,
    fd: OwnedFd,
}

/// One entry moved into a retained quarantine directory
#[derive(Debug)]
pub(super) struct QuarantinedEntry {
    name: OsString,
}

impl Quarantine {
    /// Create a mode-0700 directory beside the source entry
    pub(super) fn create(parent_fd: &OwnedFd) -> io::Result<Self> {
        // The quarantine must be beside the source so renameat can keep the original filesystem
        // semantics and preserve the exact inode instead of falling back to a data copy
        let candidates = random_names(QUARANTINE_PREFIX)?;
        for name in candidates {
            match mkdirat(parent_fd, &name, Mode::from_raw_mode(0o700)).map_err(io::Error::from) {
                Ok(()) => {
                    // Restore exact permissions after umask processing before any entry is moved
                    let fd = match open_directory_at(parent_fd, &name) {
                        Ok(fd) => fd,
                        Err(error) => {
                            remove_created_directory(parent_fd, &name);
                            return Err(error);
                        }
                    };
                    if let Err(error) = fchmod(&fd, Mode::from_raw_mode(0o700)) {
                        drop(fd);
                        remove_created_directory(parent_fd, &name);
                        return Err(error.into());
                    }
                    if let Err(error) = sync_directory(&fd) {
                        drop(fd);
                        remove_created_directory(parent_fd, &name);
                        return Err(error);
                    }
                    if let Err(error) = sync_directory(parent_fd) {
                        drop(fd);
                        remove_created_directory(parent_fd, &name);
                        return Err(error);
                    }
                    // Keep this descriptor for the entire claim, validation, and cleanup flow
                    return Ok(Self { name, fd });
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }

        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "unable to create a private quarantine directory",
        ))
    }

    /// Atomically move one source basename into the private directory
    pub(super) fn move_entry(
        &self,
        source_parent: &OwnedFd,
        source_name: &OsString,
    ) -> io::Result<QuarantinedEntry> {
        for name in random_names(ENTRY_PREFIX)? {
            // RENAME_NOREPLACE prevents a pre-existing quarantine name from being overwritten
            match renameat_with(
                source_parent,
                source_name,
                &self.fd,
                &name,
                RenameFlags::NOREPLACE,
            )
            .map_err(io::Error::from)
            {
                Ok(()) => {
                    // The source basename is now empty, so a watcher can only create a new entry
                    // there and cannot change the object that this quarantine entry represents
                    return Ok(QuarantinedEntry { name });
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }

        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "unable to reserve a quarantine entry name",
        ))
    }

    /// Restore a quarantined entry without replacing a new source entry
    pub(super) fn restore(
        &self,
        entry: &QuarantinedEntry,
        source_parent: &OwnedFd,
        source_name: &OsString,
    ) -> io::Result<()> {
        // Never restore over a replacement that appeared at the original basename
        renameat_with(
            &self.fd,
            &entry.name,
            source_parent,
            source_name,
            RenameFlags::NOREPLACE,
        )
        .map_err(io::Error::from)
    }

    /// Remove one already-validated quarantined entry
    pub(super) fn unlink(&self, entry: &QuarantinedEntry) -> io::Result<()> {
        // The caller revalidates the entry against its retained descriptor immediately before
        // this operation, which catches replacement objects during the normal claim flow
        // unlinkat still resolves entry.name at this syscall; it has no unlink-by-FD mode
        unlinkat(&self.fd, &entry.name, AtFlags::empty()).map_err(io::Error::from)?;
        // Persist the removal while the quarantine descriptor still identifies its directory
        sync_directory(&self.fd)
    }

    /// Remove the quarantine directory when no mismatched entry was retained
    pub(super) fn cleanup(self, parent_fd: &OwnedFd) -> io::Result<()> {
        let Self { name, fd } = self;
        // Directory removal is housekeeping after the claimed entry is gone, not the object claim
        drop(fd);
        match unlinkat(parent_fd, &name, AtFlags::REMOVEDIR).map_err(io::Error::from) {
            Ok(()) => sync_directory(parent_fd),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::DirectoryNotEmpty => {
                Err(io::Error::other("quarantine retained an unexpected entry"))
            }
            Err(error) => Err(error),
        }
    }

    /// Expose the retained descriptor to identity checks without exposing a pathname
    pub(super) const fn fd(&self) -> &OwnedFd {
        &self.fd
    }
}

fn remove_created_directory(parent_fd: &OwnedFd, name: &OsString) {
    // Failed setup must not leave an unused staging directory behind
    let _ = unlinkat(parent_fd, name, AtFlags::REMOVEDIR);
    let _ = sync_directory(parent_fd);
}

impl QuarantinedEntry {
    /// Return the entry name relative to the retained quarantine descriptor
    pub(super) const fn name(&self) -> &OsString {
        &self.name
    }
}

fn random_names(prefix: &str) -> io::Result<Vec<OsString>> {
    let mut random = [0_u8; RANDOM_BYTES];
    let bytes_read = getrandom(&mut random, GetRandomFlags::empty()).map_err(io::Error::from)?;
    if bytes_read != random.len() {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "secure random source returned too few bytes",
        ));
    }
    let mut token = String::with_capacity(RANDOM_BYTES.saturating_mul(2));
    for byte in random {
        write!(&mut token, "{byte:02x}")
            .map_err(|_| io::Error::other("failed to format quarantine name"))?;
    }
    let process_id = std::process::id();

    Ok((0..QUARANTINE_ATTEMPTS)
        .map(|attempt| OsString::from(format!("{prefix}{process_id}.{token}.{attempt}")))
        .collect())
}

#[cfg(test)]
#[path = "tests/quarantine.rs"]
mod tests;
