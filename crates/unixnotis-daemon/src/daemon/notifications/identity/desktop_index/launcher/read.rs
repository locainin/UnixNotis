//! Bounded descriptor-backed launcher reads

use std::fs::File;
use std::io::Read;
use std::os::fd::OwnedFd;
use std::path::Path;
use std::time::SystemTime;

use rustix::fs::{open, Mode, OFlags};

use super::super::super::executable::FileIdentity;

pub(super) const MAX_LAUNCHER_BYTES: u64 = 64 * 1024;
pub(super) const MAX_LAUNCHER_LINES: usize = 1_024;

pub(super) struct LauncherContents {
    pub(super) contents: Vec<u8>,
    pub(super) identity: FileIdentity,
    pub(super) digest: [u8; 32],
}

pub(super) fn read_launcher(
    path: &Path,
    expected_identity: FileIdentity,
) -> Option<LauncherContents> {
    // No-follow prevents a launcher path from redirecting inspection through a symlink
    let descriptor = open_launcher_descriptor(path)?;
    let mut file = File::from(descriptor);
    let before = file.metadata().ok()?;
    let identity = FileIdentity::from_metadata(&before);
    if !identity.same_file(expected_identity) {
        return None;
    }
    if !identity.is_system_managed() {
        return None;
    }
    if !identity.is_executable_regular() {
        return None;
    }
    if !launcher_size_is_supported(before.len()) {
        return None;
    }

    // One extra byte distinguishes the exact limit from a truncated oversized script
    let mut contents = Vec::with_capacity(usize::try_from(before.len()).ok()?);
    file.by_ref()
        .take(MAX_LAUNCHER_BYTES.saturating_add(1))
        .read_to_end(&mut contents)
        .ok()?;
    if !launcher_contents_are_supported(&contents) {
        return None;
    }

    // A second descriptor snapshot rejects replacement or mutation during the read
    let after = file.metadata().ok()?;
    let after_identity = FileIdentity::from_metadata(&after);
    if !snapshot_is_unchanged(
        identity,
        after_identity,
        before.len(),
        after.len(),
        before.modified().ok()?,
        after.modified().ok()?,
    ) {
        return None;
    }

    Some(LauncherContents {
        digest: *blake3::hash(&contents).as_bytes(),
        contents,
        identity,
    })
}

pub(super) const fn launcher_size_is_supported(size: u64) -> bool {
    size <= MAX_LAUNCHER_BYTES
}

pub(super) fn launcher_contents_are_supported(contents: &[u8]) -> bool {
    u64::try_from(contents.len())
        .ok()
        .is_some_and(launcher_size_is_supported)
        && contents.split(|byte| *byte == b'\n').count() <= MAX_LAUNCHER_LINES
}

pub(super) fn open_launcher_descriptor(path: &Path) -> Option<OwnedFd> {
    open(path, protected_open_flags(), Mode::empty()).ok()
}

pub(super) const fn protected_open_flags() -> OFlags {
    OFlags::RDONLY
        .union(OFlags::CLOEXEC)
        .union(OFlags::NOFOLLOW)
}

pub(super) fn snapshot_is_unchanged(
    before_identity: FileIdentity,
    after_identity: FileIdentity,
    before_size: u64,
    after_size: u64,
    before_modified: SystemTime,
    after_modified: SystemTime,
) -> bool {
    before_identity.same_file(after_identity)
        && before_size == after_size
        && before_modified == after_modified
}
