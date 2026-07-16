//! Installer-artifact paths and report construction

use std::path::PathBuf;

use unixnotis_core::service_manager::{ServiceManagerKind, ServiceManagerPaths};

use super::super::report::redact_home;
use super::super::report::{DoctorCheck, DoctorSeverity};

pub(super) const SERVICE_NAME: &str = "unixnotis-daemon";

pub(super) fn primary_artifact(paths: &ServiceManagerPaths) -> PathBuf {
    // These paths mirror the installer outputs rather than manager-wide conventions
    match paths.kind {
        ServiceManagerKind::Systemd => paths.artifact_root.join("unixnotis-daemon.service"),
        ServiceManagerKind::Dinit => paths.artifact_root.join(SERVICE_NAME),
        ServiceManagerKind::Runit => paths.artifact_root.join(SERVICE_NAME).join("run"),
        ServiceManagerKind::S6 => paths
            .artifact_root
            .join("sv")
            .join(SERVICE_NAME)
            .join("run"),
    }
}

pub(super) fn artifact_check(paths: &ServiceManagerPaths) -> DoctorCheck {
    // Only regular primary artifacts count as an installed backend signal
    let artifact = primary_artifact(paths);
    // Home redaction keeps reports safe to attach to public issues
    let display = redact_home(&artifact);
    if artifact.is_file() {
        DoctorCheck::new(
            "service.artifact",
            "Service artifact",
            DoctorSeverity::Pass,
            "Installed service artifact was found",
        )
        .details(format!(
            "Manager: {}\nArtifact: {display}",
            paths.kind.label()
        ))
        .data("manager", paths.kind.label())
        .data("artifact", display)
        .data("present", true)
    } else {
        DoctorCheck::new(
            "service.artifact",
            "Service artifact",
            DoctorSeverity::Warning,
            "Installed service artifact was not found",
        )
        .details(format!(
            "Manager: {}\nExpected: {display}",
            paths.kind.label()
        ))
        .data("manager", paths.kind.label())
        .data("artifact", display)
        .data("present", false)
    }
}
