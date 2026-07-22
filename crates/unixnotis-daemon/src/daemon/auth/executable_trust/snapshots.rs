//! Startup-time trusted executable snapshots

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

use super::super::policy::{
    TrustedExecutableSnapshot, TrustedSnapshotCacheEntry, TRUSTED_CONTROL_EXECUTABLES,
    TRUSTED_SNAPSHOT_CACHE_CAPACITY,
};
use super::fingerprint::file_fingerprint;
use super::paths::canonicalize_best_effort;

pub(in crate::daemon) fn trusted_control_snapshot(
    trusted_dir: &Path,
    executable: &str,
) -> Option<TrustedExecutableSnapshot> {
    if let Some(snapshot) = load_cached_trusted_snapshot(trusted_dir, executable) {
        return Some(snapshot);
    }

    // Pin the whole sibling trust set together so late file swaps do not sneak in
    let snapshots = build_trusted_control_snapshots(trusted_dir);
    let snapshot = snapshots.get(executable).cloned()?;
    store_cached_trusted_snapshots(trusted_dir, snapshots);
    Some(snapshot)
}

pub(in crate::daemon) fn build_trusted_control_snapshots(
    trusted_dir: &Path,
) -> HashMap<String, TrustedExecutableSnapshot> {
    let mut snapshots = HashMap::new();
    for executable in TRUSTED_CONTROL_EXECUTABLES {
        let Some(snapshot) = build_trusted_control_snapshot(trusted_dir, executable) else {
            continue;
        };
        snapshots.insert(executable.to_string(), snapshot);
    }
    snapshots
}

fn build_trusted_control_snapshot(
    trusted_dir: &Path,
    executable: &str,
) -> Option<TrustedExecutableSnapshot> {
    // Missing sibling means this executable is not trusted in strict mode
    let candidate = trusted_dir.join(executable);
    if !candidate.is_file() {
        return None;
    }

    let canonical = canonicalize_best_effort(&candidate);
    let fingerprint = file_fingerprint(&canonical)?;
    Some(TrustedExecutableSnapshot {
        canonical_path: canonical,
        fingerprint,
    })
}

pub(in crate::daemon) fn trusted_snapshot_cache() -> &'static Mutex<Vec<TrustedSnapshotCacheEntry>>
{
    static CACHE: OnceLock<Mutex<Vec<TrustedSnapshotCacheEntry>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(Vec::new()))
}

pub(in crate::daemon) fn load_cached_trusted_snapshot(
    trusted_dir: &Path,
    executable: &str,
) -> Option<TrustedExecutableSnapshot> {
    let cache = trusted_snapshot_cache();
    let cache = match cache.lock() {
        Ok(cache) => cache,
        Err(poisoned) => poisoned.into_inner(),
    };
    cache
        .iter()
        .find(|entry| entry.trusted_dir == trusted_dir)
        .and_then(|entry| entry.snapshots.get(executable).cloned())
}

pub(in crate::daemon) fn store_cached_trusted_snapshots(
    trusted_dir: &Path,
    snapshots: HashMap<String, TrustedExecutableSnapshot>,
) {
    let cache = trusted_snapshot_cache();
    let mut cache = match cache.lock() {
        Ok(cache) => cache,
        Err(poisoned) => poisoned.into_inner(),
    };

    // Replace existing directory cache before enforcing capacity
    if let Some(index) = cache
        .iter()
        .position(|entry| entry.trusted_dir == trusted_dir)
    {
        cache.remove(index);
    }
    if cache.len() >= TRUSTED_SNAPSHOT_CACHE_CAPACITY {
        cache.remove(0);
    }
    cache.push(TrustedSnapshotCacheEntry {
        trusted_dir: trusted_dir.to_path_buf(),
        snapshots,
    });
}
