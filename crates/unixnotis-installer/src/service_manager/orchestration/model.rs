//! Service-manager identity and backend-owned path state

use std::path::{Path, PathBuf};

use super::super::backends::{dinit, runit, s6, systemd};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ServiceManagerKind {
    // Each value selects one backend contract without leaking branching to callers
    Systemd,
    Dinit,
    Runit,
    S6,
}

impl ServiceManagerKind {
    const fn label(self) -> &'static str {
        match self {
            Self::Systemd => "systemd --user",
            Self::Dinit => "dinit --user",
            Self::Runit => "runit user services",
            Self::S6 => "s6-rc user services",
        }
    }
}

pub struct ServiceManager {
    // Backend identity and its owned roots travel together
    pub(super) kind: ServiceManagerKind,
    pub(super) artifact_root: PathBuf,
    live_root: Option<PathBuf>,
}

impl ServiceManager {
    pub const fn systemd_user(artifact_root: PathBuf) -> Self {
        // Systemd keeps its unit and live manager state behind one user-level root
        Self {
            kind: ServiceManagerKind::Systemd,
            artifact_root,
            live_root: None,
        }
    }

    pub const fn dinit_user(artifact_root: PathBuf) -> Self {
        // Dinit stores service definitions under the selected user configuration root
        Self {
            kind: ServiceManagerKind::Dinit,
            artifact_root,
            live_root: None,
        }
    }

    pub const fn runit_user(artifact_root: PathBuf) -> Self {
        // Runit watches the artifact root directly through the user's runsvdir
        Self {
            kind: ServiceManagerKind::Runit,
            artifact_root,
            live_root: None,
        }
    }

    pub const fn s6_user(artifact_root: PathBuf, live_root: PathBuf) -> Self {
        // s6 separates persistent source data from the compiled live database
        Self {
            kind: ServiceManagerKind::S6,
            artifact_root,
            live_root: Some(live_root),
        }
    }

    pub fn label(&self) -> &'static str {
        self.kind.label()
    }

    pub const fn service_name(&self) -> &'static str {
        // The backend owns its native service identifier
        match self.kind {
            ServiceManagerKind::Systemd => systemd::SERVICE_NAME,
            ServiceManagerKind::Dinit => dinit::SERVICE_NAME,
            ServiceManagerKind::Runit => runit::SERVICE_NAME,
            ServiceManagerKind::S6 => s6::SERVICE_NAME,
        }
    }

    pub fn artifact_label(&self) -> &'static str {
        // Artifact labels describe the manager-specific file shown in summaries
        match self.kind {
            ServiceManagerKind::Systemd => systemd::artifact_label(),
            ServiceManagerKind::Dinit => dinit::artifact_label(),
            ServiceManagerKind::Runit => runit::artifact_label(),
            ServiceManagerKind::S6 => s6::artifact_label(),
        }
    }

    pub fn manager_label(&self) -> &'static str {
        // Manager labels remain separate from short service identifiers
        match self.kind {
            ServiceManagerKind::Systemd => systemd::manager_label(),
            ServiceManagerKind::Dinit => dinit::manager_label(),
            ServiceManagerKind::Runit => runit::manager_label(),
            ServiceManagerKind::S6 => s6::manager_label(),
        }
    }

    pub fn artifact_root(&self) -> &Path {
        // Callers receive a borrowed root and cannot replace backend ownership state
        &self.artifact_root
    }

    pub fn manages_same_backend_root(&self, other: &Self) -> bool {
        // Kind plus artifact root is the real ownership identity
        self.kind == other.kind && self.artifact_root == other.artifact_root
    }

    pub(super) fn live_root(&self) -> &Path {
        // Only the s6 branches call this helper after construction supplied a live root
        self.live_root
            .as_deref()
            .expect("s6 manager should carry a live root")
    }
}
