//! Desktop record lookup tables and trusted relay matching

use std::path::{Path, PathBuf};

use super::super::executable::{executable_evidence_for_path, FileIdentity};
use super::model::{
    DesktopApplicationFamily, DesktopIdentityIndex, DesktopRecord, ExecutableIdentity,
    LaunchArgument,
};
use super::names::{normalize_brand_name, normalize_desktop_id, normalize_name};

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
                self.records
                    .iter()
                    .enumerate()
                    .filter_map(|(index, record)| {
                        (record.system_origin
                            && [&record.display_name, &record.id]
                                .iter()
                                .any(|name| normalize_brand_name(name) == protected))
                        .then_some(index)
                    }),
            );
        }
        indices.sort_unstable();
        indices.dedup();
        indices
            .into_iter()
            .filter_map(|index| self.records.get(index))
            .collect()
    }

    pub(in crate::daemon::notifications::identity) fn family_for_record(
        &self,
        record: &DesktopRecord,
    ) -> Option<&DesktopApplicationFamily> {
        let record_index = self
            .records
            .iter()
            .position(|candidate| std::ptr::eq(candidate, record))?;
        let family_index = *self.family_by_record.get(record_index)?.as_ref()?;
        self.families.get(family_index)
    }

    pub(in crate::daemon::notifications::identity) fn family_index_for_record(
        &self,
        record: &DesktopRecord,
    ) -> Option<usize> {
        let record_index = self
            .records
            .iter()
            .position(|candidate| std::ptr::eq(candidate, record))?;
        self.family_by_record.get(record_index).copied().flatten()
    }

    pub(in crate::daemon::notifications::identity) fn canonical_id_for_record<'record>(
        &'record self,
        record: &'record DesktopRecord,
    ) -> &'record str {
        self.family_for_record(record)
            .map_or(record.id.as_str(), |family| family.canonical_id.as_str())
    }

    pub(in crate::daemon::notifications::identity) fn canonical_record_for_record<'record>(
        &'record self,
        record: &'record DesktopRecord,
    ) -> &'record DesktopRecord {
        let Some(family) = self.family_for_record(record) else {
            return record;
        };
        family
            .records
            .iter()
            .filter_map(|index| self.records.get(*index))
            .find(|candidate| {
                normalize_desktop_id(&candidate.id) == normalize_desktop_id(&family.canonical_id)
            })
            .unwrap_or(record)
    }

    pub(in crate::daemon::notifications::identity) fn records_share_family(
        &self,
        left: &DesktopRecord,
        right: &DesktopRecord,
    ) -> bool {
        match (
            self.family_index_for_record(left),
            self.family_index_for_record(right),
        ) {
            (Some(left), Some(right)) => left == right,
            _ => std::ptr::eq(left, right),
        }
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

    pub(in crate::daemon::notifications::identity) fn records_form_one_application_family(
        &self,
        identity: FileIdentity,
        system_origin: bool,
    ) -> bool {
        let families = self
            .records_for_executable(identity)
            .into_iter()
            .filter(|record| record.system_origin == system_origin)
            .filter_map(|record| self.family_index_for_record(record))
            .collect::<std::collections::HashSet<_>>();
        families.len() == 1
    }

    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "async wrapper remains for test seams")
    )]
    pub(in crate::daemon::notifications::identity) async fn install_provenance_for_path_async(
        &self,
        path: PathBuf,
    ) -> super::provenance::InstallProvenance {
        self.install_provenance_for_path(path)
    }

    pub(in crate::daemon::notifications::identity) fn install_provenance_for_path(
        &self,
        path: PathBuf,
    ) -> super::provenance::InstallProvenance {
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
        for name in &record.names {
            self.by_name
                .entry(name.clone())
                .or_default()
                .push(record_index);
        }
        // Only records with a reproducible launch contract become executable evidence
        if record.association_eligible {
            if let Some(identity) = record.runtime_executable_identity {
                self.by_identity
                    .entry((identity.device, identity.inode))
                    .or_default()
                    .push(record_index);
            }
        }
        self.records.push(record);
        self.index_application_family(record_index);
    }

    pub(super) fn rebuild_executable_index(&mut self) {
        self.by_identity.clear();
        for (record_index, record) in self.records.iter().enumerate() {
            if !record.association_eligible {
                continue;
            }
            if let Some(identity) = record.runtime_executable_identity {
                self.by_identity
                    .entry((identity.device, identity.inode))
                    .or_default()
                    .push(record_index);
            }
        }
    }

    pub(super) fn rebuild_application_families(&mut self) {
        self.families.clear();
        self.family_by_record.clear();
        for record_index in 0..self.records.len() {
            self.index_application_family(record_index);
        }
    }

    fn index_application_family(&mut self, record_index: usize) {
        let Some(record) = self.records.get(record_index) else {
            self.family_by_record.push(None);
            return;
        };
        let Some(executable_identity) = record.runtime_executable_identity else {
            self.family_by_record.push(None);
            return;
        };
        let protected_payloads = protected_payload_signature(record);
        let family_index = self.families.iter().position(|family| {
            family.executable_identity.same_file(executable_identity)
                && family.system_origin == record.system_origin
                && family.system_association == record.system_association
                && family
                    .install_provenance
                    .same_application_source(&record.runtime_executable_provenance)
                && family.protected_payloads == protected_payloads
                && family_names_are_compatible(family, record)
        });

        if let Some(family_index) = family_index {
            let family = &mut self.families[family_index];
            family.records.push(record_index);
            family.names.extend(record.names.iter().cloned());
            if canonical_id_precedes(&record.id, &family.canonical_id) {
                family.canonical_id.clone_from(&record.id);
            }
            self.family_by_record.push(Some(family_index));
            return;
        }

        let family_index = self.families.len();
        self.families.push(DesktopApplicationFamily {
            canonical_id: record.id.clone(),
            executable_identity,
            records: vec![record_index],
            names: record.names.clone(),
            system_origin: record.system_origin,
            system_association: record.system_association,
            install_provenance: record.runtime_executable_provenance.clone(),
            protected_payloads,
        });
        self.family_by_record.push(Some(family_index));
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

fn protected_payload_signature(record: &DesktopRecord) -> Vec<(usize, u64, u64)> {
    record
        .launch_spec
        .iter()
        .flat_map(|spec| spec.arguments.iter().enumerate())
        .filter_map(|(position, argument)| {
            let LaunchArgument::Literal(literal) = argument else {
                return None;
            };
            let (_path, identity) = literal.file.as_ref()?;
            (!literal.value.starts_with(b"-")).then_some((
                position,
                identity.device,
                identity.inode,
            ))
        })
        .collect()
}

fn family_names_are_compatible(family: &DesktopApplicationFamily, record: &DesktopRecord) -> bool {
    if family.names.iter().any(|name| record.names.contains(name)) {
        return true;
    }
    let family_id = normalize_desktop_id(&family.canonical_id);
    let record_id = normalize_desktop_id(&record.id);
    id_is_alias_of(&family_id, &record_id)
}

fn id_is_alias_of(left: &str, right: &str) -> bool {
    left == right
        || left
            .strip_prefix(right)
            .is_some_and(|suffix| suffix.starts_with('.'))
        || right
            .strip_prefix(left)
            .is_some_and(|suffix| suffix.starts_with('.'))
}

fn canonical_id_precedes(candidate: &str, current: &str) -> bool {
    let candidate = normalize_desktop_id(candidate);
    let current = normalize_desktop_id(current);
    (candidate.len(), candidate.as_str()) < (current.len(), current.as_str())
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

#[cfg(test)]
mod tests;
