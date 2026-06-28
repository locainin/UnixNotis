//! Read-only installed-state snapshot construction

use std::path::PathBuf;

use crate::paths::InstallPaths;

use super::binaries::resolve_install_binaries_best_effort;
use super::conflicts::{detect_service_manager_conflict_state, ServiceManagerConflict};

#[derive(Clone)]
pub(in crate::actions) struct BinaryState {
    // Display name comes from cargo metadata or the fallback binary list
    pub(in crate::actions) name: String,
    // Concrete install path shown in logs when a binary is missing or present
    pub(in crate::actions) path: PathBuf,
    // Existence is enough here because binary copying owns replacement safety later
    pub(in crate::actions) exists: bool,
}

#[derive(Clone)]
pub struct InstallState {
    // Binary status is logged before service checks so users see what is missing first
    pub(in crate::actions) binaries: Vec<BinaryState>,
    // Service artifacts must match the expected safe shape, not merely exist by path
    pub(in crate::actions) service_artifact_exists: bool,
    // Enabled can be native-manager state or backend-owned artifact state
    pub(in crate::actions) service_enabled: bool,
    // Active reflects the current running state when the backend can prove it
    pub(in crate::actions) service_active: bool,
    // Probe errors are recorded for diagnostics without panicking during status rendering
    pub(in crate::actions) service_enabled_error: Option<String>,
    pub(in crate::actions) service_active_error: Option<String>,
    // Binary discovery failures should not hide already-installed files from the UI
    pub(in crate::actions) binary_warning: Option<String>,
    // Cross-backend conflicts block install before any new files are written
    pub(in crate::actions) service_conflicts: Vec<ServiceManagerConflict>,
    // Optional backend scan warnings stay visible without blocking the selected manager
    pub(in crate::actions) service_conflict_warnings: Vec<String>,
}

impl InstallState {
    pub fn is_installed(&self) -> bool {
        // Treat installed as binaries plus the service artifact; runtime status is separate
        !self.binaries.is_empty()
            && self.binaries.iter().all(|binary| binary.exists)
            && self.service_artifact_exists
    }

    pub fn is_fully_installed(&self) -> bool {
        self.is_installed() && self.service_active
    }

    pub fn service_enabled(&self) -> bool {
        self.service_enabled
    }
}

pub fn check_install_state(paths: &InstallPaths) -> InstallState {
    // Keep install state aligned with installer binary discovery.
    // Best-effort resolution keeps install state usable even if workspace metadata is broken.
    let (binaries, warning) = resolve_install_binaries_best_effort(paths);
    let binaries = binaries
        .into_iter()
        .map(|name| {
            let path = paths.bin_dir.join(&name);
            BinaryState {
                name,
                exists: path.exists(),
                path,
            }
        })
        .collect::<Vec<_>>();

    let service_artifact_exists = paths
        .service
        .artifacts(&paths.bin_dir)
        .iter()
        // Artifact presence must prove the expected file, directory marker, or link target
        .all(crate::service_manager::ServiceArtifact::is_present_safely);
    // Enabled state decides whether reinstall can skip `enable --now`
    // Some backends store enablement as installer-owned artifacts instead of manager state
    let mut service_enabled_error = None;
    let service_enabled = if let Some(enabled) = paths.service.enabled_by_artifacts() {
        // Artifact-backed managers prove enablement through installer-owned filesystem state
        enabled
    } else {
        match paths.service.is_enabled_command() {
            Some(spec) => match spec.to_command().status() {
                // Command-backed managers still use the native manager status probe
                Ok(status) => status.success(),
                Err(err) => {
                    service_enabled_error = Some(err.to_string());
                    false
                }
            },
            None => {
                // This should only apply to future backends that have no state probe yet
                service_enabled_error =
                    Some("service manager has no enabled-state command".to_string());
                false
            }
        }
    };
    // Active state still matters for the install summary shown in the UI
    let mut service_active_error = None;
    let service_active = match paths.service.active_probe() {
        Some(probe) => match probe.evaluate() {
            // Active probes can be plain exit status or stdout parsing, depending on backend
            Ok(active) => active,
            Err(err) => {
                service_active_error = Some(err.to_string());
                false
            }
        },
        None => {
            // Backends without active state still allow install, but cannot claim a running service
            service_active_error = Some("service manager has no active-state command".to_string());
            false
        }
    };

    let (service_conflicts, service_conflict_warnings) =
        detect_service_manager_conflict_state(paths);

    InstallState {
        binaries,
        service_artifact_exists,
        service_enabled,
        service_active,
        service_enabled_error,
        service_active_error,
        binary_warning: warning,
        service_conflicts,
        service_conflict_warnings,
    }
}
