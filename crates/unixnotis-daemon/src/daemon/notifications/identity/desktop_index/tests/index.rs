//! Executable lookup-index rebuild cases

use std::collections::HashSet;

use super::super::index::{portal_candidate_paths, portal_identity_is_trusted};
use super::super::model::{DesktopIdentityIndex, DesktopRecord, LaunchSpec};
use crate::daemon::notifications::identity::desktop_index::provenance::InstallProvenance;
use crate::daemon::notifications::identity::executable::{
    executable_evidence_for_path, FileIdentity,
};
use crate::test_support::TempRoot;

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

#[test]
fn portal_discovery_filters_before_applying_the_candidate_limit() {
    let root = TempRoot::new("portal-discovery-filter-order");
    for index in 0..300 {
        std::fs::write(
            root.join(format!("ordinary-library-{index:03}")),
            b"fixture",
        )
        .expect("write non-portal directory entry");
    }
    let portal = root.join("xdg-desktop-portal-example");
    std::fs::write(&portal, b"portal fixture").expect("write portal directory entry");

    let candidates = portal_candidate_paths(root.path());

    assert_eq!(candidates, vec![portal]);
}

#[test]
fn portal_identity_requires_both_system_management_and_executable_file_type() {
    let trusted = FileIdentity {
        device: 1,
        inode: 2,
        uid: 0,
        mode: 0o100_755,
    };
    assert!(portal_identity_is_trusted(trusted));
    assert!(!portal_identity_is_trusted(FileIdentity {
        uid: 1_000,
        ..trusted
    }));
    assert!(!portal_identity_is_trusted(FileIdentity {
        mode: 0o100_644,
        ..trusted
    }));
}

#[test]
fn installed_protected_portal_is_indexed_when_available() {
    let installed = [
        "/usr/lib",
        "/usr/libexec",
        "/usr/local/lib",
        "/usr/local/libexec",
    ]
    .into_iter()
    .find_map(|directory| {
        portal_candidate_paths(std::path::Path::new(directory))
            .into_iter()
            .find_map(|path| {
                let evidence = executable_evidence_for_path(&path)?;
                portal_identity_is_trusted(evidence.identity).then_some((path, evidence.identity))
            })
    });
    let Some((portal, identity)) = installed else {
        // Platforms without an installed portal backend have no system fixture to index
        return;
    };
    let directory = portal.parent().expect("installed portal parent directory");
    let mut index = DesktopIdentityIndex::default();

    index.index_trusted_portals_in(directory);

    assert!(index
        .trusted_portals
        .iter()
        .any(|candidate| candidate.identity.same_file(identity)));
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
