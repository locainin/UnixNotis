use unixnotis_core::service_manager::{ServiceManagerKind, ServiceManagerPaths};

use super::super::manager::{manager_artifact_exists, select_detected_manager};
use super::support::TempToolDir;

#[test]
fn automatic_manager_selection_accepts_exactly_one_installed_service() {
    assert_eq!(
        select_detected_manager(&[ServiceManagerKind::Runit]).expect("one manager"),
        ServiceManagerKind::Runit
    );
}

#[test]
fn automatic_manager_selection_rejects_no_installed_service() {
    let error = select_detected_manager(&[]).expect_err("missing service must be rejected");

    assert!(error
        .to_string()
        .contains("no installed UnixNotis user service"));
}

#[test]
fn automatic_manager_selection_rejects_ambiguous_installed_services() {
    assert!(
        select_detected_manager(&[ServiceManagerKind::Systemd, ServiceManagerKind::Dinit]).is_err()
    );
}

#[test]
fn manager_artifact_detection_accepts_only_expected_files_or_directories() {
    let root = TempToolDir::new("manager-artifacts");
    let runit = ServiceManagerPaths {
        kind: ServiceManagerKind::Runit,
        artifact_root: root.path().join("runit"),
        live_root: None,
    };

    assert!(!manager_artifact_exists(&runit));
    root.create_dir("runit/unixnotis-daemon");
    assert!(manager_artifact_exists(&runit));

    let systemd = ServiceManagerPaths {
        kind: ServiceManagerKind::Systemd,
        artifact_root: root.path().join("systemd"),
        live_root: None,
    };
    root.write_file("systemd/unixnotis-daemon.service", "[Unit]\n");
    assert!(manager_artifact_exists(&systemd));
}
