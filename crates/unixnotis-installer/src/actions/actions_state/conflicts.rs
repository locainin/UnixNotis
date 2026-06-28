//! Cross-backend service-manager conflict detection

use std::path::PathBuf;

use crate::paths::InstallPaths;

#[derive(Clone)]
pub(in crate::actions) struct ServiceManagerConflict {
    // User-facing manager name for the backend that appears to own UnixNotis already
    pub(in crate::actions) manager_label: &'static str,
    // Artifact wording stays backend-specific so errors are clear for s6/runit directories
    pub(in crate::actions) artifact_label: &'static str,
    // Primary artifact path gives the user one concrete place to inspect
    pub(in crate::actions) artifact_path: PathBuf,
    // Installed means every steady artifact for the other backend matches the safe shape
    pub(in crate::actions) installed: bool,
    // Active means the other backend's native runtime probe says its daemon is running
    pub(in crate::actions) active: bool,
}

pub(in crate::actions) fn detect_service_manager_conflict_state(
    paths: &InstallPaths,
) -> (Vec<ServiceManagerConflict>, Vec<String>) {
    let mut conflicts = Vec::new();
    let mut warnings = Vec::new();

    // Selected-backend reinstall is valid, but sibling backends must not keep owning the daemon
    for manager in paths.alternate_service_managers() {
        let manager = match manager {
            Ok(manager) => manager,
            Err(err) => {
                // A broken non-selected backend path should be visible but should not block install
                warnings.push(err.to_string());
                continue;
            }
        };

        // Artifact ownership uses the same safe shape checks as selected-backend state
        let artifacts = manager.artifacts(&paths.bin_dir);
        let installed = !artifacts.is_empty()
            && artifacts
                .iter()
                .all(crate::service_manager::ServiceArtifact::is_present_safely);
        // Active probes are best-effort because missing tools should not become false conflicts
        let active = manager
            .active_probe()
            .and_then(|probe| probe.evaluate().ok())
            .unwrap_or(false);

        // Only real evidence should block install; probe errors are treated as not active
        if installed || active {
            conflicts.push(ServiceManagerConflict {
                manager_label: manager.label(),
                artifact_label: manager.artifact_label(),
                artifact_path: manager.primary_artifact_path(),
                installed,
                active,
            });
        }
    }

    (conflicts, warnings)
}
