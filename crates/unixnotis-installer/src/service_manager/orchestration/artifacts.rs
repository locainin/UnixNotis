//! Artifact creation, refresh, and readiness dispatch

use std::path::{Path, PathBuf};

use super::super::backends::{dinit, runit, s6, systemd};
use super::super::contract::{ReadinessIssue, ServiceArtifact, ServiceArtifactRefresh};
use super::model::{ServiceManager, ServiceManagerKind};

impl ServiceManager {
    pub fn primary_artifact_path(&self) -> PathBuf {
        // Summaries use one stable path even when a backend owns several files
        match self.kind {
            ServiceManagerKind::Systemd => systemd::primary_artifact_path(&self.artifact_root),
            ServiceManagerKind::Dinit => dinit::primary_artifact_path(&self.artifact_root),
            ServiceManagerKind::Runit => runit::primary_artifact_path(&self.artifact_root),
            ServiceManagerKind::S6 => s6::primary_artifact_path(&self.artifact_root),
        }
    }

    pub fn artifacts(&self, bin_dir: &Path) -> Vec<ServiceArtifact> {
        // This set describes the stable state left after installation completes
        match self.kind {
            ServiceManagerKind::Systemd => systemd::artifacts(&self.artifact_root, bin_dir),
            ServiceManagerKind::Dinit => dinit::artifacts(&self.artifact_root, bin_dir),
            ServiceManagerKind::Runit => runit::artifacts(&self.artifact_root, bin_dir),
            ServiceManagerKind::S6 => s6::artifacts(&self.artifact_root, bin_dir),
        }
    }

    pub fn install_artifacts(&self, bin_dir: &Path) -> Vec<ServiceArtifact> {
        // Runit adds a temporary start gate during installation
        match self.kind {
            ServiceManagerKind::Systemd => systemd::artifacts(&self.artifact_root, bin_dir),
            ServiceManagerKind::Dinit => dinit::artifacts(&self.artifact_root, bin_dir),
            ServiceManagerKind::Runit => runit::install_artifacts(&self.artifact_root, bin_dir),
            ServiceManagerKind::S6 => s6::artifacts(&self.artifact_root, bin_dir),
        }
    }

    pub fn refresh_after_artifact_change(&self) -> Option<ServiceArtifactRefresh> {
        // s6 returns a compile plan while simpler managers return one reload command
        match self.kind {
            ServiceManagerKind::Systemd => Some(ServiceArtifactRefresh::Command(
                systemd::reload_after_artifact_change(),
            )),
            ServiceManagerKind::Dinit => {
                dinit::reload_after_artifact_change().map(ServiceArtifactRefresh::Command)
            }
            ServiceManagerKind::Runit => {
                runit::reload_after_artifact_change().map(ServiceArtifactRefresh::Command)
            }
            ServiceManagerKind::S6 => Some(s6::refresh_after_artifact_change(
                &self.artifact_root,
                self.live_root(),
            )),
        }
    }

    pub fn pre_start_artifacts_to_remove(&self) -> Vec<ServiceArtifact> {
        // Runit removes the down gate only after environment files are ready
        match self.kind {
            ServiceManagerKind::Runit => runit::pre_start_artifacts_to_remove(&self.artifact_root),
            ServiceManagerKind::Systemd | ServiceManagerKind::Dinit | ServiceManagerKind::S6 => {
                Vec::new()
            }
        }
    }

    pub fn pre_start_artifacts_to_write(&self) -> Vec<ServiceArtifact> {
        // Install artifacts already carry the runit gate so no backend writes one here
        match self.kind {
            ServiceManagerKind::Runit => runit::pre_start_artifacts_to_write(&self.artifact_root),
            ServiceManagerKind::Systemd | ServiceManagerKind::Dinit | ServiceManagerKind::S6 => {
                Vec::new()
            }
        }
    }

    pub fn readiness_issues(&self) -> Vec<ReadinessIssue> {
        // Readiness reports missing tools or roots without mutating manager state
        match self.kind {
            ServiceManagerKind::Systemd => Vec::new(),
            ServiceManagerKind::Dinit => dinit::readiness_issues(&self.artifact_root),
            ServiceManagerKind::Runit => runit::readiness_issues(),
            ServiceManagerKind::S6 => s6::readiness_issues(&self.artifact_root, self.live_root()),
        }
    }
}
