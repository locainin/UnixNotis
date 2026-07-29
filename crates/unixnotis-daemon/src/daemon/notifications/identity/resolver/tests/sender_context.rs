//! Live sender-context enrichment tests

use super::super::sender_context::enrich_sender_install_provenance;
use super::*;

#[tokio::test]
async fn provenance_enrichment_preserves_known_ownership_without_lookup() {
    let expected = package("example-app");
    let mut metadata = SenderMetadata {
        install_provenance: expected.clone(),
        ..SenderMetadata::default()
    };

    enrich_sender_install_provenance(&mut metadata, &DesktopIdentityIndex::default()).await;

    assert_eq!(metadata.install_provenance, expected);
}

#[tokio::test]
async fn provenance_enrichment_keeps_unknown_when_process_identity_is_missing() {
    let mut metadata = SenderMetadata::default();

    enrich_sender_install_provenance(&mut metadata, &DesktopIdentityIndex::default()).await;

    assert_eq!(metadata.install_provenance, InstallProvenance::Unknown);
}

#[tokio::test]
async fn provenance_enrichment_resolves_a_reopened_system_executable() {
    let (path, executable_identity) = installed_system_executable();
    let mut metadata = sender(&path, executable_identity);

    enrich_sender_install_provenance(&mut metadata, &DesktopIdentityIndex::default()).await;

    assert!(metadata.install_provenance.is_known());
}

#[tokio::test]
async fn provenance_enrichment_rejects_untrusted_or_nonexecutable_sender_metadata() {
    let (path, executable_identity) = installed_system_executable();
    let invalid_identities = [
        FileIdentity {
            uid: 1_000,
            ..executable_identity
        },
        FileIdentity {
            mode: 0o100_644,
            ..executable_identity
        },
    ];

    for invalid_identity in invalid_identities {
        let mut metadata = sender(&path, invalid_identity);
        enrich_sender_install_provenance(&mut metadata, &DesktopIdentityIndex::default()).await;
        assert_eq!(metadata.install_provenance, InstallProvenance::Unknown);
    }
}

#[tokio::test]
async fn provenance_enrichment_rejects_a_stale_executable_identity() {
    let (path, executable_identity) = installed_system_executable();
    let stale_identity = FileIdentity {
        inode: executable_identity.inode.saturating_add(1),
        ..executable_identity
    };
    let mut metadata = sender(&path, stale_identity);

    enrich_sender_install_provenance(&mut metadata, &DesktopIdentityIndex::default()).await;

    assert_eq!(metadata.install_provenance, InstallProvenance::Unknown);
}
