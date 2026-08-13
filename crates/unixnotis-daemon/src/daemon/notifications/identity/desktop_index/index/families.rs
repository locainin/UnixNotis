//! Application-family construction and canonical identity selection

use super::super::super::executable::FileIdentity;
use super::super::model::{
    DesktopApplicationFamily, DesktopIdentityIndex, DesktopRecord, LaunchArgument,
};
use super::super::names::normalize_desktop_id;

impl DesktopIdentityIndex {
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

    pub(in crate::daemon::notifications::identity) fn rebuild_application_families(&mut self) {
        self.families.clear();
        self.family_by_record.clear();
        for record_index in 0..self.records.len() {
            self.index_application_family(record_index);
        }
    }

    pub(super) fn index_application_family(&mut self, record_index: usize) {
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
