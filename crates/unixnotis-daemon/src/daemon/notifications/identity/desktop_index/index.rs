//! Desktop record lookup tables and trusted relay matching

use std::path::Path;
#[cfg(test)]
use std::path::PathBuf;

use super::super::executable::{executable_evidence_for_path, FileIdentity};
use super::model::{DesktopIdentityIndex, DesktopRecord, ExecutableIdentity};
use super::names::{normalize_brand_name, normalize_desktop_id};

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

    #[cfg(test)]
    pub(in crate::daemon::notifications::identity) fn from_records(
        records: Vec<DesktopRecord>,
        trusted_relays: Vec<(PathBuf, FileIdentity)>,
    ) -> Self {
        let mut index = Self::default();
        for record in records {
            index.index_record(record);
        }
        index.trusted_relays = trusted_relays
            .into_iter()
            .map(|(path, identity)| ExecutableIdentity { path, identity })
            .collect();
        index
    }

    pub(super) fn index_record(&mut self, record: DesktopRecord) {
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
