use std::path::PathBuf;

use crate::service_manager::{ServiceArtifact, ServiceArtifactKind};

use super::service_artifacts_are_present;

#[test]
fn empty_service_artifact_list_is_not_installed() {
    // A backend with no artifacts has not proved ownership of anything on disk
    assert!(!service_artifacts_are_present(&[]));
}

#[test]
fn missing_service_artifact_list_is_not_installed() {
    let artifact = ServiceArtifact {
        // Use a fixed missing path because this test only needs the safe-presence negative path
        path: PathBuf::from("/tmp/unixnotis-missing-service-artifact"),
        kind: ServiceArtifactKind::File,
        contents: Some(String::new()),
        mode: None,
    };

    // Non-empty lists still need every artifact to match the expected safe shape
    assert!(!service_artifacts_are_present(&[artifact]));
}
