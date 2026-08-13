use super::{BinaryState, InstallState, InstallationDisposition};
use crate::actions::releases::BinaryHealth;
use crate::service_manager::{ServiceArtifact, ServiceArtifactKind};

use super::service_artifacts_are_present;

#[test]
fn empty_service_artifact_list_is_not_installed() {
    // A backend with no artifacts has not proved ownership of anything on disk
    assert!(!service_artifacts_are_present(&[]));
}

#[test]
fn installation_disposition_labels_are_distinct_and_actionable() {
    assert_eq!(
        InstallationDisposition::NotInstalled.label(),
        "not installed"
    );
    assert_eq!(InstallationDisposition::InstalledHealthy.label(), "healthy");
    assert_eq!(
        InstallationDisposition::RepairRequired.label(),
        "repair required"
    );
}

#[test]
fn missing_service_artifact_list_is_not_installed() {
    let artifact = ServiceArtifact {
        // Use a fixed missing path because this test only needs the safe-presence negative path
        path: std::env::temp_dir().join("unixnotis-missing-service-artifact"),
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
            path: std::env::temp_dir().join("unixnotis-daemon"),
            health: BinaryHealth::Healthy {
                generation: "test-generation".to_string(),
                package_version: "1.2.0".to_string(),
                digest: "test-digest".to_string(),
            },
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
    assert_eq!(
        base.disposition(),
        InstallationDisposition::InstalledHealthy
    );
    assert_eq!(base.installed_version(), Some("1.2.0"));

    let mut no_binaries = base.clone();
    no_binaries.binaries.clear();
    assert!(!no_binaries.is_installed());
    assert_eq!(
        no_binaries.disposition(),
        InstallationDisposition::RepairRequired
    );

    let mut missing_binary = base.clone();
    missing_binary.binaries[0].health = BinaryHealth::Missing;
    assert!(!missing_binary.is_installed());
    assert_eq!(
        missing_binary.disposition(),
        InstallationDisposition::RepairRequired
    );

    let mut missing_service = base;
    missing_service.service_artifact_exists = false;
    assert!(!missing_service.is_installed());
    assert_eq!(
        missing_service.disposition(),
        InstallationDisposition::RepairRequired
    );
}

#[test]
fn install_state_with_no_binary_or_service_footprint_is_not_installed() {
    let state = InstallState {
        binaries: vec![BinaryState {
            name: "unixnotis-daemon".to_string(),
            path: std::env::temp_dir().join("unixnotis-daemon"),
            health: BinaryHealth::Missing,
        }],
        service_artifact_exists: false,
        service_enabled: false,
        service_active: false,
        service_enabled_error: None,
        service_active_error: None,
        binary_warning: None,
        service_conflicts: Vec::new(),
        service_conflict_warnings: Vec::new(),
    };

    assert_eq!(state.disposition(), InstallationDisposition::NotInstalled);
}

#[test]
fn indeterminate_selected_manager_requires_repair_for_present_binaries() {
    let state = InstallState {
        binaries: vec![BinaryState {
            name: "unixnotis-daemon".to_string(),
            path: std::env::temp_dir().join("unixnotis-daemon"),
            health: BinaryHealth::Healthy {
                generation: "test-generation".to_string(),
                package_version: "1.2.0".to_string(),
                digest: "test-digest".to_string(),
            },
        }],
        service_artifact_exists: true,
        service_enabled: false,
        service_active: false,
        service_enabled_error: None,
        service_active_error: Some("manager state is indeterminate".to_string()),
        binary_warning: None,
        service_conflicts: Vec::new(),
        service_conflict_warnings: Vec::new(),
    };

    assert!(!state.is_installed());
    assert_eq!(state.disposition(), InstallationDisposition::RepairRequired);
}

#[test]
fn fully_installed_requires_running_service_and_enabled_accessor_tracks_field() {
    let mut state = InstallState {
        binaries: vec![BinaryState {
            name: "unixnotis-daemon".to_string(),
            path: std::env::temp_dir().join("unixnotis-daemon"),
            health: BinaryHealth::Healthy {
                generation: "test-generation".to_string(),
                package_version: "1.2.0".to_string(),
                digest: "test-digest".to_string(),
            },
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
    assert!(state.service_enabled);
    assert!(!state.is_fully_installed());

    state.service_active = true;

    assert!(state.is_fully_installed());
}

#[test]
fn install_state_rejects_individually_healthy_binaries_from_different_generations() {
    let binary = |name: &str, generation: &str| BinaryState {
        name: name.to_string(),
        path: std::env::temp_dir().join(name),
        health: BinaryHealth::Healthy {
            generation: generation.to_string(),
            package_version: "1.2.0".to_string(),
            digest: format!("digest-{name}"),
        },
    };
    let state = InstallState {
        binaries: vec![
            binary("unixnotis-daemon", "generation-a"),
            binary("unixnotis-center", "generation-b"),
        ],
        service_artifact_exists: true,
        service_enabled: true,
        service_active: true,
        service_enabled_error: None,
        service_active_error: None,
        binary_warning: None,
        service_conflicts: Vec::new(),
        service_conflict_warnings: Vec::new(),
    };

    assert!(
        !state.is_installed(),
        "different release generations must never form one installed state"
    );
    assert_eq!(
        state.disposition(),
        InstallationDisposition::RepairRequired,
        "mixed release generations must be presented as a repair"
    );
}
