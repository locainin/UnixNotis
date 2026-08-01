//! Executable lookup-index mutation cases

use std::collections::HashSet;

use super::super::super::model::{DesktopIdentityIndex, DesktopRecord, LaunchSpec};
use crate::daemon::notifications::identity::desktop_index::provenance::InstallProvenance;
use crate::daemon::notifications::identity::executable::FileIdentity;

#[test]
fn executable_index_rebuild_replaces_stale_runtime_identity() {
    let old = identity(70);
    let new = identity(71);
    let mut index = DesktopIdentityIndex::default();
    index.index_record(record(old));
    index.records[0].runtime_executable_identity = Some(new);
    index.records[0]
        .launch_spec
        .as_mut()
        .expect("runtime launch specification")
        .runtime_executable = new;

    index.rebuild_executable_index();

    assert!(index.records_for_executable(old).is_empty());
    assert_eq!(index.records_for_executable(new).len(), 1);
}

fn record(runtime: FileIdentity) -> DesktopRecord {
    DesktopRecord {
        id: "org.example.App".to_string(),
        display_name: "Example App".to_string(),
        badge_icon: "example-app".to_string(),
        desktop_path: None,
        declared_executable_path: Some("/usr/bin/example-app".into()),
        declared_executable_identity: Some(runtime),
        runtime_executable_path: Some("/usr/bin/example-app".into()),
        runtime_executable_identity: Some(runtime),
        desktop_identity: None,
        desktop_provenance: InstallProvenance::Unknown,
        declared_executable_provenance: InstallProvenance::Unknown,
        runtime_executable_provenance: InstallProvenance::Unknown,
        system_origin: false,
        system_association: false,
        association_eligible: true,
        launch_spec: Some(LaunchSpec {
            declared_executable: runtime,
            runtime_executable: runtime,
            arguments: Vec::new(),
            environment: Vec::new(),
            wrappers: Vec::new(),
            package_launcher: None,
            literal_files_are_system_managed: false,
        }),
        names: HashSet::new(),
    }
}

fn identity(inode: u64) -> FileIdentity {
    FileIdentity {
        device: 1,
        inode,
        uid: 1_000,
        mode: 0o100_755,
    }
}
