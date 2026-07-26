//! Shared authorization policy constants and small data records

use std::collections::HashMap;
use std::path::PathBuf;

// Only these sibling binaries may call privileged control methods
pub(in crate::daemon) const TRUSTED_CONTROL_EXECUTABLES: [&str; 4] = [
    "noticenterctl",
    "unixnotis-center",
    "unixnotis-popups",
    "unixnotis-daemon",
];

// Only the center process may publish panel readiness state
pub(in crate::daemon) const TRUSTED_PANEL_READINESS_EXECUTABLES: [&str; 1] = ["unixnotis-center"];

// Only the popup renderer may publish its composite D-Bus and GTK readiness
pub(in crate::daemon) const TRUSTED_POPUP_READINESS_EXECUTABLES: [&str; 1] = ["unixnotis-popups"];

// Small bounded caches avoid unbounded growth from repeated forged callers
pub(in crate::daemon) const FINGERPRINT_CACHE_CAPACITY: usize = 32;
pub(in crate::daemon) const TRUSTED_SNAPSHOT_CACHE_CAPACITY: usize = 32;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::daemon) struct TrustedExecutableSnapshot {
    // Canonical path ties trust to one concrete on-disk binary
    pub(in crate::daemon) canonical_path: PathBuf,
    // Fingerprint blocks same-path replacement after daemon startup
    pub(in crate::daemon) fingerprint: FileFingerprint,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::daemon) struct FileFingerprint {
    // Metadata signature catches path swaps and in-place rewrites cheaply
    pub(in crate::daemon) signature: FileFingerprintSignature,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::daemon) struct FileFingerprintSignature {
    // File length is portable and always included in the signature
    pub(in crate::daemon) len: u64,
    #[cfg(unix)]
    pub(in crate::daemon) dev: u64,
    #[cfg(unix)]
    pub(in crate::daemon) ino: u64,
    #[cfg(unix)]
    pub(in crate::daemon) mode: u32,
    #[cfg(unix)]
    pub(in crate::daemon) uid: u32,
    #[cfg(unix)]
    pub(in crate::daemon) gid: u32,
    #[cfg(unix)]
    pub(in crate::daemon) mtime: i64,
    #[cfg(unix)]
    pub(in crate::daemon) mtime_nsec: i64,
    #[cfg(unix)]
    pub(in crate::daemon) ctime: i64,
    #[cfg(unix)]
    pub(in crate::daemon) ctime_nsec: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::daemon) struct FingerprintCacheEntry {
    pub(in crate::daemon) path: PathBuf,
    pub(in crate::daemon) signature: FileFingerprintSignature,
    pub(in crate::daemon) fingerprint: FileFingerprint,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::daemon) struct TrustedSnapshotCacheEntry {
    pub(in crate::daemon) trusted_dir: PathBuf,
    pub(in crate::daemon) snapshots: HashMap<String, TrustedExecutableSnapshot>,
}
