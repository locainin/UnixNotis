//! Read-only lookups over the constructed desktop index

use std::path::PathBuf;

use super::super::super::executable::FileIdentity;
use super::super::model::{DesktopIdentityIndex, DesktopRecord};
use super::super::names::{normalize_brand_name, normalize_desktop_id, normalize_name};

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

    pub(in crate::daemon::notifications::identity) fn records_for_claim(
        &self,
        claim: &str,
    ) -> Vec<&DesktopRecord> {
        let normalized = normalize_name(claim);
        let mut indices = self
            .by_name
            .get(&normalized)
            .into_iter()
            .flatten()
            .copied()
            .collect::<Vec<_>>();

        // Protected confusable names still resolve to concrete system candidates
        let protected = normalize_brand_name(claim);
        if !protected.is_empty() {
            indices.extend(
                self.system_brand_records
                    .get(&protected)
                    .into_iter()
                    .flatten()
                    .copied(),
            );
        }
        indices.sort_unstable();
        indices.dedup();
        indices
            .into_iter()
            .filter_map(|index| self.records.get(index))
            .collect()
    }

    pub(in crate::daemon::notifications::identity) fn record_matches_claim(
        &self,
        record: &DesktopRecord,
        claim: &str,
    ) -> bool {
        let normalized = normalize_name(claim);
        record.claim_matches(claim)
            || self
                .family_for_record(record)
                .is_some_and(|family| family.names.contains(&normalized))
            || self
                .records_for_claim(claim)
                .iter()
                .any(|candidate| std::ptr::eq(*candidate, record))
    }

    pub(in crate::daemon::notifications::identity) fn install_provenance_for_path(
        &self,
        path: PathBuf,
    ) -> super::super::provenance::InstallProvenance {
        // The caller owns the attribution worker permit while this blocking lookup runs
        self.package_ownership.resolve_one(&path)
    }

    pub(in crate::daemon::notifications::identity) fn claim_matches_system_app(
        &self,
        claim: &str,
    ) -> bool {
        // Confusable spellings share one protected-brand skeleton
        let claim = normalize_brand_name(claim);
        !claim.is_empty() && self.system_brand_names.contains(&claim)
    }
}
