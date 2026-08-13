//! Stable executable identity captured from open file metadata

use std::fs::{File, Metadata};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub(in crate::daemon) struct FileIdentity {
    pub(super) device: u64,
    pub(super) inode: u64,
    pub(super) uid: u32,
    pub(super) mode: u32,
}

impl FileIdentity {
    pub(super) fn from_metadata(metadata: &Metadata) -> Self {
        Self {
            device: metadata.dev(),
            inode: metadata.ino(),
            uid: metadata.uid(),
            mode: metadata.mode(),
        }
    }

    pub(super) const fn same_file(self, other: Self) -> bool {
        // Device and inode survive symlink aliases and ordinary path spelling changes
        self.device == other.device && self.inode == other.inode
    }

    pub(super) const fn is_system_managed(self) -> bool {
        // Same-user attackers cannot replace a root-owned non-writable file
        self.uid == 0 && self.mode & 0o022 == 0
    }

    pub(super) const fn is_executable_regular(self) -> bool {
        // Authority binaries must be regular files with at least one execute bit
        self.mode & 0o170_000 == 0o100_000 && self.mode & 0o111 != 0
    }

    pub(super) fn group_fragment(self) -> String {
        // Group keys expose no path while remaining stable for the running file
        format!("{}:{}", self.device, self.inode)
    }
}

#[derive(Debug, Clone)]
pub(in crate::daemon) struct ExecutableEvidence {
    pub(in crate::daemon) canonical_path: PathBuf,
    pub(in crate::daemon) identity: FileIdentity,
}

pub(in crate::daemon) fn executable_evidence_for_pid(pid: u32) -> Option<ExecutableEvidence> {
    let proc_executable = PathBuf::from(format!("/proc/{pid}/exe"));
    // Opening the procfs link binds metadata to the running file instead of a mutable path
    let file = File::open(&proc_executable).ok()?;
    let identity = FileIdentity::from_metadata(&file.metadata().ok()?);
    let live_path = std::fs::read_link(&proc_executable).ok()?;
    if live_path.as_os_str().as_bytes().ends_with(b" (deleted)") {
        // Deleted mappings no longer have a protected installed path to revalidate
        return None;
    }
    let canonical_path = proc_executable
        .canonicalize()
        .or(Ok::<PathBuf, std::io::Error>(live_path))
        .ok()?;
    Some(ExecutableEvidence {
        canonical_path,
        identity,
    })
}

pub(super) fn executable_evidence_for_path(path: &Path) -> Option<ExecutableEvidence> {
    // Open-file metadata prevents a path replacement from changing the checked identity
    let file = File::open(path).ok()?;
    let identity = FileIdentity::from_metadata(&file.metadata().ok()?);
    let canonical_path = path.canonicalize().ok()?;
    Some(ExecutableEvidence {
        canonical_path,
        identity,
    })
}

#[cfg(test)]
#[path = "tests/executable.rs"]
mod tests;
