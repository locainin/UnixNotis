//! Fail-closed cross-backend service-manager conflict detection

use std::path::{Path, PathBuf};

use crate::paths::InstallPaths;
use crate::service_manager::contract::{ServiceManagerAvailability, ServiceProbeState};
use crate::service_manager::{ServiceArtifactState, ServiceManager};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(in crate::actions) enum ServiceManagerConflictKind {
    Active,
    Installed,
    PartialInstall,
    UnsafeArtifact,
    Indeterminate,
}

#[derive(Clone)]
pub(in crate::actions) struct ServiceManagerConflict {
    pub(in crate::actions) manager_label: &'static str,
    pub(in crate::actions) artifact_label: &'static str,
    pub(in crate::actions) artifact_path: PathBuf,
    pub(in crate::actions) kinds: Vec<ServiceManagerConflictKind>,
    pub(in crate::actions) artifact_paths: Vec<PathBuf>,
    pub(in crate::actions) detail: Option<String>,
}

struct ArtifactInspection {
    expected: Vec<PathBuf>,
    missing: usize,
    unsafe_paths: Vec<PathBuf>,
    error: Option<String>,
}

enum RuntimeInspection {
    Unavailable,
    State(ServiceProbeState),
    Indeterminate(String),
}

pub(in crate::actions) fn detect_service_manager_conflict_state(
    paths: &InstallPaths,
) -> (Vec<ServiceManagerConflict>, Vec<String>) {
    let mut conflicts = Vec::new();

    for manager in paths.alternate_service_managers() {
        let manager = match manager {
            Ok(manager) => manager,
            Err(error) => {
                // An invalid alternate root is unknown ownership state, never proof of absence
                conflicts.push(ServiceManagerConflict {
                    manager_label: "alternate service manager",
                    artifact_label: "service artifacts",
                    artifact_path: PathBuf::new(),
                    kinds: vec![ServiceManagerConflictKind::Indeterminate],
                    artifact_paths: Vec::new(),
                    detail: Some(error.to_string()),
                });
                continue;
            }
        };

        let inspection = inspect_artifacts(&manager, &paths.bin_dir);
        let mut kinds = Vec::new();
        add_artifact_conflict_kind(&inspection, &mut kinds);
        let mut inspection_error = inspection.error;
        let runtime = inspect_runtime(&manager);
        if matches!(runtime, RuntimeInspection::Unavailable) && kinds.is_empty() {
            // No transport and no artifacts means this alternate backend owns nothing here
            continue;
        }
        add_runtime_conflict_kind(runtime, manager.label(), &mut kinds, &mut inspection_error);
        // An unavailable or absent manager with no artifacts owns no live UnixNotis service
        // Existing artifacts still retain their installed, partial, or unsafe conflict kind
        if kinds.is_empty() {
            continue;
        }

        let mut artifact_paths = inspection.expected;
        artifact_paths.extend(inspection.unsafe_paths);
        artifact_paths.sort();
        artifact_paths.dedup();
        conflicts.push(ServiceManagerConflict {
            manager_label: manager.label(),
            artifact_label: manager.artifact_label(),
            artifact_path: manager.primary_artifact_path(),
            kinds,
            artifact_paths,
            detail: inspection_error,
        });
    }

    // Indeterminate states are conflicts now, so no fail-open warning channel remains
    (conflicts, Vec::new())
}

fn inspect_artifacts(manager: &ServiceManager, bin_dir: &Path) -> ArtifactInspection {
    let mut inspection = ArtifactInspection {
        expected: Vec::new(),
        missing: 0,
        unsafe_paths: Vec::new(),
        error: None,
    };
    for artifact in manager.artifacts(bin_dir) {
        match artifact.inspect() {
            Ok(ServiceArtifactState::Expected) => inspection.expected.push(artifact.path),
            Ok(ServiceArtifactState::Missing) => {
                inspection.missing = inspection.missing.saturating_add(1);
            }
            Ok(ServiceArtifactState::UnexpectedObject) => {
                inspection.unsafe_paths.push(artifact.path);
            }
            Err(error) => {
                inspection.error = Some(format!(
                    "could not inspect {} at {}: {error}",
                    manager.artifact_label(),
                    artifact.path.display()
                ));
                break;
            }
        }
    }
    inspection
}

fn add_artifact_conflict_kind(
    inspection: &ArtifactInspection,
    kinds: &mut Vec<ServiceManagerConflictKind>,
) {
    let kind = if inspection.error.is_some() {
        Some(ServiceManagerConflictKind::Indeterminate)
    } else if !inspection.unsafe_paths.is_empty() {
        Some(ServiceManagerConflictKind::UnsafeArtifact)
    } else if !inspection.expected.is_empty() && inspection.missing == 0 {
        Some(ServiceManagerConflictKind::Installed)
    } else if inspection.expected.is_empty() {
        None
    } else {
        Some(ServiceManagerConflictKind::PartialInstall)
    };
    kinds.extend(kind);
}

fn inspect_runtime(manager: &ServiceManager) -> RuntimeInspection {
    match manager.availability_state() {
        Ok(Some(ServiceManagerAvailability::Unavailable)) => RuntimeInspection::Unavailable,
        Ok(Some(ServiceManagerAvailability::Available) | None) => {
            match manager.active_probe().evaluate_state() {
                Ok(state) => RuntimeInspection::State(state),
                Err(error) => RuntimeInspection::Indeterminate(format!(
                    "could not establish whether {} is active: {error}",
                    manager.label()
                )),
            }
        }
        Ok(Some(ServiceManagerAvailability::Indeterminate)) => {
            RuntimeInspection::Indeterminate(format!(
                "{} returned an indeterminate manager availability state",
                manager.label()
            ))
        }
        Err(error) => RuntimeInspection::Indeterminate(format!(
            "could not establish whether {} is reachable: {error}",
            manager.label()
        )),
    }
}

fn add_runtime_conflict_kind(
    runtime: RuntimeInspection,
    manager_label: &str,
    kinds: &mut Vec<ServiceManagerConflictKind>,
    detail: &mut Option<String>,
) {
    let runtime_detail = match runtime {
        RuntimeInspection::State(ServiceProbeState::Active) => {
            kinds.push(ServiceManagerConflictKind::Active);
            None
        }
        RuntimeInspection::State(ServiceProbeState::Indeterminate) => {
            kinds.push(ServiceManagerConflictKind::Indeterminate);
            Some(format!(
                "{manager_label} returned an indeterminate service state"
            ))
        }
        RuntimeInspection::Indeterminate(message) => {
            kinds.push(ServiceManagerConflictKind::Indeterminate);
            Some(message)
        }
        RuntimeInspection::Unavailable
        | RuntimeInspection::State(
            ServiceProbeState::Unavailable
            | ServiceProbeState::Absent
            | ServiceProbeState::Inactive,
        ) => None,
    };
    if detail.is_none() {
        *detail = runtime_detail;
    }
    kinds.sort_unstable();
    kinds.dedup();
}
