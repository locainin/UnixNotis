use std::path::PathBuf;

use unixnotis_core::service_manager::{ServiceManagerKind, ServiceManagerPaths};

use super::super::artifacts::primary_artifact;

fn paths(kind: ServiceManagerKind) -> ServiceManagerPaths {
    ServiceManagerPaths {
        kind,
        artifact_root: PathBuf::from("/tmp/unixnotis-service-artifacts"),
        live_root: (kind == ServiceManagerKind::S6)
            .then(|| PathBuf::from("/tmp/unixnotis-service-live")),
    }
}

#[test]
fn primary_artifacts_follow_each_installer_layout() {
    assert_eq!(
        primary_artifact(&paths(ServiceManagerKind::Systemd)),
        PathBuf::from("/tmp/unixnotis-service-artifacts/unixnotis-daemon.service")
    );
    assert_eq!(
        primary_artifact(&paths(ServiceManagerKind::Dinit)),
        PathBuf::from("/tmp/unixnotis-service-artifacts/unixnotis-daemon")
    );
    assert_eq!(
        primary_artifact(&paths(ServiceManagerKind::Runit)),
        PathBuf::from("/tmp/unixnotis-service-artifacts/unixnotis-daemon/run")
    );
    assert_eq!(
        primary_artifact(&paths(ServiceManagerKind::S6)),
        PathBuf::from("/tmp/unixnotis-service-artifacts/sv/unixnotis-daemon/run")
    );
}
