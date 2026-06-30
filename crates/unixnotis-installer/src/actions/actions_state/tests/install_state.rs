use std::path::PathBuf;

use super::{BinaryState, InstallState};
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

#[test]
fn install_state_requires_non_empty_binary_list_all_binaries_and_service_artifact() {
    let base = InstallState {
        binaries: vec![BinaryState {
            name: "unixnotis-daemon".to_string(),
            path: PathBuf::from("/tmp/unixnotis-daemon"),
            exists: true,
        }],
        service_artifact_exists: true,
        service_enabled: false,
        service_active: false,
        service_enabled_error: None,
        service_active_error: None,
        binary_warning: None,
        service_conflicts: Vec::new(),
        service_conflict_warnings: Vec::new(),
    };

    // Full install state needs at least one binary, every binary present, and a safe service artifact
    assert!(base.is_installed());

    let mut no_binaries = base.clone();
    no_binaries.binaries.clear();
    assert!(!no_binaries.is_installed());

    let mut missing_binary = base.clone();
    missing_binary.binaries[0].exists = false;
    assert!(!missing_binary.is_installed());

    let mut missing_service = base;
    missing_service.service_artifact_exists = false;
    assert!(!missing_service.is_installed());
}

#[test]
fn fully_installed_requires_running_service_and_enabled_accessor_tracks_field() {
    let mut state = InstallState {
        binaries: vec![BinaryState {
            name: "unixnotis-daemon".to_string(),
            path: PathBuf::from("/tmp/unixnotis-daemon"),
            exists: true,
        }],
        service_artifact_exists: true,
        service_enabled: true,
        service_active: false,
        service_enabled_error: None,
        service_active_error: None,
        binary_warning: None,
        service_conflicts: Vec::new(),
        service_conflict_warnings: Vec::new(),
    };

    // Enabled state and active state are separate; install summary should not conflate them
    assert!(state.is_installed());
    assert!(state.service_enabled());
    assert!(!state.is_fully_installed());

    state.service_active = true;

    assert!(state.is_fully_installed());
}
