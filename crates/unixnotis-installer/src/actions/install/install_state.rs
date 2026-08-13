//! Read-only installed-state snapshot construction

use std::path::PathBuf;

use crate::paths::InstallPaths;
use crate::service_manager::ServiceArtifact;

use super::super::binaries::resolve_install_binaries_best_effort;
use super::super::conflicts::{detect_service_manager_conflict_state, ServiceManagerConflict};
use super::super::releases::{inspect_installed_generation, BinaryHealth};

#[derive(Clone)]
pub(in crate::actions) struct BinaryState {
    // Display name comes from cargo metadata or the fallback binary list
    pub(in crate::actions) name: String,
    // Concrete install path shown in logs when a binary is missing or present
    pub(in crate::actions) path: PathBuf,
    // Read-side health uses the same generation, link, type, size, and digest invariants as install
    pub(in crate::actions) health: BinaryHealth,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InstallationDisposition {
    // No selected-manager artifact or managed binary entrypoint was found
    NotInstalled,
    // Every binary belongs to one verified generation and manager state is trustworthy
    InstalledHealthy,
    // A managed footprint exists but at least one required health invariant failed
    RepairRequired,
}

impl InstallationDisposition {
    pub const fn label(self) -> &'static str {
        match self {
            Self::NotInstalled => "not installed",
            Self::InstalledHealthy => "healthy",
            Self::RepairRequired => "repair required",
        }
    }
}

impl InstallState {
    pub fn is_installed(&self) -> bool {
        // Healthy means both filesystem integrity and manager inspection are trustworthy
        self.healthy_generation().is_some()
            && self.service_artifact_exists
            && self.service_enabled_error.is_none()
            && self.service_active_error.is_none()
            && self.service_conflicts.is_empty()
    }

    pub fn is_fully_installed(&self) -> bool {
        self.is_installed() && self.service_active
    }

    pub fn disposition(&self) -> InstallationDisposition {
        if self.is_installed() {
            InstallationDisposition::InstalledHealthy
        } else if self.has_installation_footprint() {
            InstallationDisposition::RepairRequired
        } else {
            InstallationDisposition::NotInstalled
        }
    }

    pub fn installed_version(&self) -> Option<&str> {
        self.healthy_generation().map(|(_, version)| version)
    }

    fn has_installation_footprint(&self) -> bool {
        self.service_artifact_exists
            || self
                .binaries
                .iter()
                .any(|binary| !matches!(binary.health, BinaryHealth::Missing))
    }

    fn healthy_generation(&self) -> Option<(&str, &str)> {
        let BinaryHealth::Healthy {
            generation,
            package_version,
            ..
        } = &self.binaries.first()?.health
        else {
            return None;
        };
        // A set of individually valid binaries is still invalid when generations differ
        self.binaries
            .iter()
            .all(|binary| {
                matches!(
                    &binary.health,
                    BinaryHealth::Healthy {
                        generation: candidate_generation,
                        package_version: candidate_version,
                        ..
                    } if candidate_generation == generation && candidate_version == package_version
                )
            })
            .then_some((generation, package_version))
    }
}

pub fn check_install_state(paths: &InstallPaths) -> InstallState {
    // Keep install state aligned with installer binary discovery
    // Best-effort resolution keeps install state usable even if workspace metadata is broken
    let (binaries, warning) = resolve_install_binaries_best_effort(paths);
    let binaries = inspect_installed_generation(paths, &binaries)
        .into_iter()
        .map(|(name, health)| BinaryState {
            path: paths.bin_dir.join(&name),
            name,
            health,
        })
        .collect::<Vec<_>>();

    // Capture the artifact list once so the empty-list guard and shape checks share one view
    let service_artifacts = paths.service.artifacts(&paths.bin_dir);
    let service_artifact_exists = service_artifacts_are_present(&service_artifacts);
    // Enabled state decides whether reinstall can skip `enable --now`
    // Some backends store enablement as installer-owned artifacts instead of manager state
    let mut service_enabled_error = None;
    let service_enabled = paths.service.enabled_by_artifacts().unwrap_or_else(|| {
        if let Some(spec) = paths.service.is_enabled_command() {
            match spec.to_command().and_then(|mut command| command.status()) {
                // Command-backed managers still use the native manager status probe
                Ok(status) => status.success(),
                Err(err) => {
                    service_enabled_error = Some(err.to_string());
                    false
                }
            }
        } else {
            // This should only apply to future backends that have no state probe yet
            service_enabled_error =
                Some("service manager has no enabled-state command".to_string());
            false
        }
    });
    // Active state still matters for the install summary shown in the UI
    let mut service_active_error = None;
    let service_active = match paths.service.active_probe().evaluate_state() {
        Ok(crate::service_manager::contract::ServiceProbeState::Active) => true,
        Ok(
            crate::service_manager::contract::ServiceProbeState::Absent
            | crate::service_manager::contract::ServiceProbeState::Inactive,
        ) => false,
        Ok(crate::service_manager::contract::ServiceProbeState::Unavailable) => {
            service_active_error = Some("selected service manager is unavailable".to_string());
            false
        }
        Ok(crate::service_manager::contract::ServiceProbeState::Indeterminate) => {
            service_active_error =
                Some("selected service manager state is indeterminate".to_string());
            false
        }
        Err(err) => {
            service_active_error = Some(err.to_string());
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

fn service_artifacts_are_present(artifacts: &[ServiceArtifact]) -> bool {
    // Empty artifact lists are never a real install, even though Iterator::all would return true
    !artifacts.is_empty()
        && artifacts
            .iter()
            // Artifact presence must prove the expected file, directory marker, or link target
            .all(ServiceArtifact::is_present_safely)
}

#[cfg(test)]
#[path = "tests/install_state.rs"]
mod tests;
