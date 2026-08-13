use std::collections::HashSet;

use crate::daemon::notifications::identity::desktop_index::model::{DesktopRecord, LaunchSpec};
use crate::daemon::notifications::identity::desktop_index::provenance::PackageProvider;
use crate::daemon::notifications::identity::desktop_index::InstallProvenance;
use crate::daemon::notifications::identity::sender::{CommandLineEvidence, CommandLineQuality};

pub(super) fn record_for_spec(id: &str, spec: &LaunchSpec) -> DesktopRecord {
    DesktopRecord {
        id: id.to_string(),
        display_name: "Contract application".to_string(),
        badge_icon: "contract".to_string(),
        desktop_path: Some(format!("/usr/share/applications/{id}.desktop").into()),
        declared_executable_path: Some("/usr/bin/true".into()),
        declared_executable_identity: Some(spec.declared_executable),
        runtime_executable_path: Some("/usr/bin/true".into()),
        runtime_executable_identity: Some(spec.runtime_executable),
        desktop_identity: None,
        desktop_provenance: test_package(id),
        declared_executable_provenance: test_package(id),
        runtime_executable_provenance: test_package(id),
        system_origin: true,
        system_association: true,
        association_eligible: true,
        launch_spec: Some(spec.clone()),
        names: HashSet::new(),
    }
}

pub(super) fn test_package(package_id: &str) -> InstallProvenance {
    InstallProvenance::Package {
        provider: PackageProvider::Pacman,
        package_id: package_id.to_string(),
    }
}

pub(super) fn structured_command(arguments: &[&str]) -> CommandLineEvidence {
    CommandLineEvidence {
        argv: arguments
            .iter()
            .map(|argument| argument.as_bytes().to_vec())
            .collect(),
        quality: CommandLineQuality::Structured,
    }
}
