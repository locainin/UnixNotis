//! Process-wide installer action serialization

use std::fs::{self, File};
use std::os::unix::fs::MetadataExt;
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use rustix::fs::{flock, open, FlockOperation, Mode, OFlags};
use rustix::process::geteuid;

const INSTALLER_LOCK_FILE: &str = "unixnotis-installer.lock";

#[derive(Debug)]
pub struct InstallerLock {
    // Retaining the descriptor retains the kernel lock for the complete action
    _file: File,
}

impl InstallerLock {
    pub fn acquire_for_session() -> Result<Self> {
        let runtime_dir = std::env::var_os("XDG_RUNTIME_DIR")
            .ok_or_else(|| anyhow!("XDG_RUNTIME_DIR is required for installer serialization"))?;
        let runtime_dir = fs::canonicalize(runtime_dir)
            .context("resolve the session runtime directory for installer serialization")?;
        let metadata = fs::metadata(&runtime_dir)
            .context("inspect the session runtime directory for installer serialization")?;
        if !owned_expected_object(metadata.is_dir(), metadata.uid(), geteuid().as_raw()) {
            return Err(anyhow!(
                "session runtime directory is not an owned directory"
            ));
        }
        Self::acquire_at(&runtime_dir.join(INSTALLER_LOCK_FILE))
    }

    fn acquire_at(path: &Path) -> Result<Self> {
        // NOFOLLOW prevents a lock-file link from redirecting the retained descriptor
        let descriptor = open(
            path,
            OFlags::RDWR
                .union(OFlags::CREATE)
                .union(OFlags::CLOEXEC)
                .union(OFlags::NOFOLLOW),
            Mode::RUSR.union(Mode::WUSR),
        )
        .context("open the installer action lock")?;
        let file = File::from(descriptor);
        let metadata = file
            .metadata()
            .context("inspect the installer action lock")?;
        if !owned_expected_object(metadata.is_file(), metadata.uid(), geteuid().as_raw()) {
            return Err(anyhow!(
                "installer action lock is not an owned regular file"
            ));
        }
        flock(&file, FlockOperation::NonBlockingLockExclusive)
            .map_err(|error| anyhow!(error))
            .context("another UnixNotis installer action is already running")?;
        Ok(Self { _file: file })
    }
}

const fn owned_expected_object(expected_kind: bool, actual_uid: u32, effective_uid: u32) -> bool {
    expected_kind && actual_uid == effective_uid
}

#[cfg(test)]
#[path = "tests/installer_lock.rs"]
mod tests;
