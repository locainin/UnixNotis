//! Fingerprint cache for trusted executable files

use std::path::Path;
use std::sync::{Mutex, OnceLock};

use super::super::policy::{
    FileFingerprint, FileFingerprintSignature, FingerprintCacheEntry, FINGERPRINT_CACHE_CAPACITY,
};
use super::metadata::trusted_control_file_metadata_is_safe;

pub(in crate::daemon) fn file_fingerprint(path: &Path) -> Option<FileFingerprint> {
    let metadata = std::fs::metadata(path).ok()?;
    if !metadata.is_file() {
        return None;
    }
    if !trusted_control_file_metadata_is_safe(&metadata) {
        return None;
    }
    let signature = file_fingerprint_signature(&metadata)?;
    if let Some(cached) = load_cached_fingerprint(path, signature) {
        return Some(cached);
    }

    // Metadata signature is fast and still detects replacement and rewrite events
    let fingerprint = FileFingerprint { signature };
    store_cached_fingerprint(path, signature, fingerprint.clone());
    Some(fingerprint)
}

pub(in crate::daemon) fn file_fingerprint_signature(
    metadata: &std::fs::Metadata,
) -> Option<FileFingerprintSignature> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;

        Some(FileFingerprintSignature {
            len: metadata.len(),
            dev: metadata.dev(),
            ino: metadata.ino(),
            mode: metadata.mode(),
            uid: metadata.uid(),
            gid: metadata.gid(),
            mtime: metadata.mtime(),
            mtime_nsec: metadata.mtime_nsec(),
            ctime: metadata.ctime(),
            ctime_nsec: metadata.ctime_nsec(),
        })
    }

    #[cfg(not(unix))]
    {
        Some(FileFingerprintSignature {
            len: metadata.len(),
        })
    }
}

pub(in crate::daemon) fn fingerprint_cache() -> &'static Mutex<Vec<FingerprintCacheEntry>> {
    static CACHE: OnceLock<Mutex<Vec<FingerprintCacheEntry>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(Vec::new()))
}

pub(in crate::daemon) fn load_cached_fingerprint(
    path: &Path,
    signature: FileFingerprintSignature,
) -> Option<FileFingerprint> {
    let cache = fingerprint_cache();
    let cache = match cache.lock() {
        Ok(cache) => cache,
        Err(poisoned) => poisoned.into_inner(),
    };
    cache
        .iter()
        .find(|entry| entry.path == path && entry.signature == signature)
        .map(|entry| entry.fingerprint.clone())
}

pub(in crate::daemon) fn store_cached_fingerprint(
    path: &Path,
    signature: FileFingerprintSignature,
    fingerprint: FileFingerprint,
) {
    let cache = fingerprint_cache();
    let mut cache = match cache.lock() {
        Ok(cache) => cache,
        Err(poisoned) => poisoned.into_inner(),
    };

    // Refresh existing entries so the oldest slot still represents real idle data
    if let Some(index) = cache.iter().position(|entry| entry.path == path) {
        cache.remove(index);
    }
    if cache.len() >= FINGERPRINT_CACHE_CAPACITY {
        cache.remove(0);
    }
    cache.push(FingerprintCacheEntry {
        path: path.to_path_buf(),
        signature,
        fingerprint,
    });
}
