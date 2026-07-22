use std::collections::HashMap;

use super::super::fingerprint::{
    fingerprint_cache, load_cached_fingerprint, store_cached_fingerprint,
};
use super::super::snapshots::{
    load_cached_trusted_snapshot, store_cached_trusted_snapshots, trusted_snapshot_cache,
};
use crate::daemon::auth::policy::{
    TrustedExecutableSnapshot, FINGERPRINT_CACHE_CAPACITY, TRUSTED_SNAPSHOT_CACHE_CAPACITY,
};
use crate::daemon::auth::support::{test_fingerprint, test_signature};
use crate::test_support::{env_lock, TempRoot};

#[test]
fn fingerprint_cache_loads_only_same_path_and_signature() {
    let _guard = env_lock();
    fingerprint_cache().lock().expect("cache lock").clear();
    let root = TempRoot::new("auth-fingerprint-cache");
    let path = root.join("noticenterctl");
    let other = root.join("unixnotis-center");
    let signature = test_signature(10);
    let fingerprint = test_fingerprint(10);

    store_cached_fingerprint(&path, signature, fingerprint.clone());

    assert_eq!(load_cached_fingerprint(&path, signature), Some(fingerprint));
    assert!(load_cached_fingerprint(&other, signature).is_none());
    assert!(load_cached_fingerprint(&path, test_signature(11)).is_none());
}

#[test]
fn fingerprint_cache_replaces_same_path_and_evicts_oldest_entry() {
    let _guard = env_lock();
    fingerprint_cache().lock().expect("cache lock").clear();
    let root = TempRoot::new("auth-fingerprint-evict");
    let path = root.join("noticenterctl");

    store_cached_fingerprint(&path, test_signature(1), test_fingerprint(1));
    store_cached_fingerprint(&path, test_signature(2), test_fingerprint(2));
    assert!(load_cached_fingerprint(&path, test_signature(1)).is_none());
    assert_eq!(
        load_cached_fingerprint(&path, test_signature(2)),
        Some(test_fingerprint(2))
    );

    for index in 0..FINGERPRINT_CACHE_CAPACITY {
        let entry_path = root.join(format!("tool-{index}"));
        store_cached_fingerprint(
            &entry_path,
            test_signature(100 + index as u64),
            test_fingerprint(100 + index as u64),
        );
    }

    assert!(load_cached_fingerprint(&path, test_signature(2)).is_none());
}

#[test]
fn trusted_snapshot_cache_loads_replaces_and_evicts_by_directory() {
    let _guard = env_lock();
    trusted_snapshot_cache()
        .lock()
        .expect("snapshot cache lock")
        .clear();
    let root = TempRoot::new("auth-snapshot-cache");
    let first_dir = root.join("first");
    let second_dir = root.join("second");
    let ctl_path = first_dir.join("noticenterctl");
    let center_path = first_dir.join("unixnotis-center");
    let first_snapshot = TrustedExecutableSnapshot {
        canonical_path: ctl_path,
        fingerprint: test_fingerprint(1),
    };
    let replacement_snapshot = TrustedExecutableSnapshot {
        canonical_path: center_path,
        fingerprint: test_fingerprint(2),
    };
    let mut snapshots = HashMap::new();
    snapshots.insert("noticenterctl".to_string(), first_snapshot.clone());

    store_cached_trusted_snapshots(&first_dir, snapshots);
    assert_eq!(
        load_cached_trusted_snapshot(&first_dir, "noticenterctl"),
        Some(first_snapshot)
    );
    assert!(load_cached_trusted_snapshot(&second_dir, "noticenterctl").is_none());

    let mut replacement = HashMap::new();
    replacement.insert("noticenterctl".to_string(), replacement_snapshot.clone());
    store_cached_trusted_snapshots(&first_dir, replacement);
    assert_eq!(
        load_cached_trusted_snapshot(&first_dir, "noticenterctl"),
        Some(replacement_snapshot)
    );

    for index in 0..TRUSTED_SNAPSHOT_CACHE_CAPACITY {
        let dir = root.join(format!("dir-{index}"));
        let mut snapshots = HashMap::new();
        snapshots.insert(
            "noticenterctl".to_string(),
            TrustedExecutableSnapshot {
                canonical_path: dir.join("noticenterctl"),
                fingerprint: test_fingerprint(100 + index as u64),
            },
        );
        store_cached_trusted_snapshots(&dir, snapshots);
    }

    assert!(load_cached_trusted_snapshot(&first_dir, "noticenterctl").is_none());
}
