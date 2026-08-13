//! Trusted portal discovery and identity cases

use super::super::super::model::DesktopIdentityIndex;
use super::super::trusted::{portal_candidate_paths, portal_identity_is_trusted};
use crate::daemon::notifications::identity::executable::executable_evidence_for_path;
use crate::daemon::notifications::identity::executable::FileIdentity;
use crate::test_support::TempRoot;

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
