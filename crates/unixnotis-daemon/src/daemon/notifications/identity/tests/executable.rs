use std::os::unix::fs::MetadataExt;

use super::*;

#[test]
fn system_managed_identity_requires_root_ownership_without_shared_writes() {
    let protected = FileIdentity {
        device: 1,
        inode: 2,
        uid: 0,
        mode: 0o100_755,
    };

    assert!(protected.is_system_managed());
    assert!(!FileIdentity {
        uid: 1000,
        ..protected
    }
    .is_system_managed());
    assert!(!FileIdentity {
        mode: 0o100_775,
        ..protected
    }
    .is_system_managed());
    assert!(!FileIdentity {
        mode: 0o100_757,
        ..protected
    }
    .is_system_managed());
}

#[test]
fn same_file_uses_device_and_inode_instead_of_mutable_labels() {
    let first = FileIdentity {
        device: 5,
        inode: 8,
        uid: 0,
        mode: 0o100_755,
    };
    let relabeled = FileIdentity {
        uid: 1000,
        mode: 0o100_777,
        ..first
    };

    assert!(first.same_file(relabeled));
    assert!(!first.same_file(FileIdentity { inode: 9, ..first }));
}

#[test]
fn executable_path_evidence_matches_open_file_metadata() {
    let executable = std::env::current_exe().expect("current test executable path");
    let evidence = executable_evidence_for_path(&executable).expect("current executable evidence");
    let metadata = std::fs::metadata(&executable).expect("current executable metadata");

    assert!(evidence.canonical_path.is_absolute());
    assert_eq!(evidence.identity.device, metadata.dev());
    assert_eq!(evidence.identity.inode, metadata.ino());
}

#[test]
fn missing_executable_path_has_no_identity_evidence() {
    assert!(executable_evidence_for_path(std::path::Path::new(
        "/path/that/does/not/exist/unixnotis"
    ))
    .is_none());
}
