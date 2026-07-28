use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::process::{Command, Stdio};

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
fn executable_regular_identity_rejects_directories_and_missing_execute_bits() {
    let executable = FileIdentity {
        device: 1,
        inode: 2,
        uid: 0,
        mode: 0o100_755,
    };
    assert!(executable.is_executable_regular());
    assert!(!FileIdentity {
        mode: 0o100_644,
        ..executable
    }
    .is_executable_regular());
    assert!(!FileIdentity {
        mode: 0o040_755,
        ..executable
    }
    .is_executable_regular());
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

#[test]
fn deleted_running_executable_has_no_trusted_identity_evidence() {
    let root = crate::test_support::TempRoot::new("deleted-running-executable");
    let source = unixnotis_core::util::trusted_system_program_path("sleep")
        .expect("find protected sleep executable");
    let executable = root.join("temporary-sleep");
    std::fs::copy(source, &executable).expect("copy sleep executable");
    std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o755))
        .expect("make copied executable runnable");
    let mut child = Command::new(&executable)
        .arg("30")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn copied executable");
    std::fs::remove_file(&executable).expect("unlink running executable");

    let evidence = executable_evidence_for_pid(child.id());

    child.kill().expect("stop copied executable");
    child.wait().expect("reap copied executable");
    assert!(evidence.is_none());
}
