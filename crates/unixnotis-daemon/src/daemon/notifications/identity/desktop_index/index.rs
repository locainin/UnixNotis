//! Desktop record lookup tables and trusted relay matching

use std::path::Path;

use super::super::executable::{executable_evidence_for_path, FileIdentity};
use super::model::{DesktopIdentityIndex, DesktopRecord, ExecutableIdentity};
use super::names::{is_shared_launcher, normalize_brand_name, normalize_desktop_id};

impl DesktopIdentityIndex {
    pub(in crate::daemon::notifications::identity) fn records_for_id(
        &self,
        id: &str,
    ) -> Vec<&DesktopRecord> {
        // Duplicate IDs remain separate so origin can be checked by the resolver
        self.by_id
            .get(&normalize_desktop_id(id))
            .into_iter()
            .flatten()
            .filter_map(|index| self.records.get(*index))
            .collect()
    }

    pub(in crate::daemon::notifications::identity) fn records_for_executable(
        &self,
        identity: FileIdentity,
    ) -> Vec<&DesktopRecord> {
        // Device and inode avoid trusting a replaceable executable path
        self.by_identity
            .get(&(identity.device, identity.inode))
            .into_iter()
            .flatten()
            .filter_map(|index| self.records.get(*index))
            .collect()
    }

    pub(in crate::daemon::notifications::identity) fn requires_launch_arguments(
        &self,
        record: &DesktopRecord,
    ) -> bool {
        let Some(identity) = record.executable_identity else {
            return true;
        };
        let Some(path) = record.executable_path.as_deref() else {
            return true;
        };
        // Generic runtimes need their fixed payload because the binary is not the application
        if is_shared_launcher(path) {
            return true;
        }

        let record_id = normalize_desktop_id(&record.id);
        // One binary serving distinct desktop applications needs argv to select the right record
        self.records_for_executable(identity)
            .iter()
            .any(|candidate| normalize_desktop_id(&candidate.id) != record_id)
    }

    pub(in crate::daemon::notifications::identity) fn claim_matches_system_app(
        &self,
        claim: &str,
    ) -> bool {
        // Confusable spellings share one protected-brand skeleton
        let claim = normalize_brand_name(claim);
        !claim.is_empty() && self.system_brand_names.contains(&claim)
    }

    pub(in crate::daemon::notifications::identity) fn has_system_record_for_id(
        &self,
        id: &str,
    ) -> bool {
        self.records_for_id(id)
            .iter()
            .any(|record| record.system_origin)
    }

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
        identity: FileIdentity,
    ) -> Option<&Path> {
        self.trusted_portals
            .iter()
            .find(|portal| portal.identity.same_file(identity))
            .map(|portal| portal.path.as_path())
    }

    pub(super) fn index_trusted_relay(&mut self, path: &Path) {
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

    pub(super) fn index_trusted_portals_in(&mut self, directory: &Path) {
        const MAX_PORTAL_CANDIDATES: usize = 256;

        let Ok(entries) = std::fs::read_dir(directory) else {
            return;
        };
        for entry in entries.take(MAX_PORTAL_CANDIDATES).flatten() {
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            if !name.starts_with("xdg-desktop-portal") {
                continue;
            }
            let Some(evidence) = executable_evidence_for_path(&path) else {
                continue;
            };
            // Portal authority is accepted only from protected system integration binaries
            if evidence.identity.is_system_managed() && evidence.identity.is_executable_regular() {
                self.trusted_portals.push(ExecutableIdentity {
                    path: evidence.canonical_path,
                    identity: evidence.identity,
                });
            }
        }
    }

    pub(in crate::daemon::notifications::identity) fn index_record(
        &mut self,
        record: DesktopRecord,
    ) {
        let record_index = self.records.len();
        if record.system_origin {
            // Protected branding excludes generic names and launcher aliases
            for brand in [&record.display_name, &record.id] {
                let brand = normalize_brand_name(brand);
                if !brand.is_empty() {
                    self.system_brand_names.insert(brand);
                }
            }
        }
        self.by_id
            .entry(normalize_desktop_id(&record.id))
            .or_default()
            .push(record_index);
        // Generic launchers are presentation records but never executable evidence
        if record.association_eligible {
            if let Some(identity) = record.executable_identity {
                self.by_identity
                    .entry((identity.device, identity.inode))
                    .or_default()
                    .push(record_index);
            }
        }
        self.records.push(record);
    }
}
