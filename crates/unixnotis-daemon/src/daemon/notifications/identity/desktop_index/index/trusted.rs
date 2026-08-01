//! Trusted relay and portal integration boundaries

use std::path::{Path, PathBuf};

use super::super::super::executable::{executable_evidence_for_path, FileIdentity};
use super::super::model::{DesktopIdentityIndex, ExecutableIdentity};

impl DesktopIdentityIndex {
    pub(in crate::daemon::notifications::identity) fn trusted_relay_path(
        &self,
        identity: FileIdentity,
    ) -> Option<&Path> {
        self.trusted_relays
            .iter()
            .find(|relay| relay.identity.same_file(identity))
            .map(|relay| relay.path.as_path())
    }

    pub(in crate::daemon::notifications::identity) fn trusted_portal_path(
        &self,
        sender_identity: FileIdentity,
        sender_path: &Path,
    ) -> Option<&Path> {
        self.trusted_portals
            .iter()
            .find(|portal| {
                let Some(current) = executable_evidence_for_path(&portal.path) else {
                    return false;
                };
                // Both the running path and installed path must remain under protected roots
                trusted_system_executable_path(sender_path)
                    && trusted_system_executable_path(&current.canonical_path)
                    && current.canonical_path == portal.path
                    && current.identity.same_file(portal.identity)
                    && current.identity.same_file(sender_identity)
                    && current.identity.is_system_managed()
                    && current.identity.is_executable_regular()
            })
            .map(|portal| portal.path.as_path())
    }

    pub(in crate::daemon::notifications::identity) fn index_trusted_relay(&mut self, path: &Path) {
        let Some(evidence) = executable_evidence_for_path(path) else {
            return;
        };
        // Writable relay binaries stay ordinary unknown senders
        if evidence.identity.is_system_managed() {
            self.trusted_relays.push(ExecutableIdentity {
                path: evidence.canonical_path,
                identity: evidence.identity,
            });
        }
    }

    pub(in crate::daemon::notifications::identity) fn index_trusted_portals_in(
        &mut self,
        directory: &Path,
    ) {
        for path in portal_candidate_paths(directory) {
            let Some(evidence) = executable_evidence_for_path(&path) else {
                continue;
            };
            // Portal authority is accepted only from protected system integration binaries
            if portal_identity_is_trusted(evidence.identity) {
                self.trusted_portals.push(ExecutableIdentity {
                    path: evidence.canonical_path,
                    identity: evidence.identity,
                });
            }
        }
    }
}

pub(in crate::daemon::notifications::identity) const fn portal_identity_is_trusted(
    identity: FileIdentity,
) -> bool {
    identity.is_system_managed() && identity.is_executable_regular()
}

pub(in crate::daemon::notifications::identity) fn portal_candidate_paths(
    directory: &Path,
) -> Vec<PathBuf> {
    const MAX_PORTAL_CANDIDATES: usize = 256;

    let Ok(entries) = std::fs::read_dir(directory) else {
        return Vec::new();
    };
    // Walk every entry in the directory
    // Only entries with a matching name count toward the cap
    // Filtering first means a directory full of unrelated files cannot hide a real portal
    entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("xdg-desktop-portal"))
                .then_some(path)
        })
        .take(MAX_PORTAL_CANDIDATES)
        .collect()
}

fn trusted_system_executable_path(path: &Path) -> bool {
    const ROOTS: [&str; 8] = [
        "/bin",
        "/lib",
        "/lib64",
        "/usr/bin",
        "/usr/lib",
        "/usr/libexec",
        "/usr/local/lib",
        "/usr/local/libexec",
    ];

    path.is_absolute() && ROOTS.iter().any(|root| path.starts_with(root))
}
