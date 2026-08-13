//! Mutation of desktop and executable indexes

use super::super::model::{DesktopIdentityIndex, DesktopRecord};
use super::super::names::{normalize_brand_name, normalize_desktop_id};

impl DesktopIdentityIndex {
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
                    self.system_brand_names.insert(brand.clone());
                    self.system_brand_records
                        .entry(brand)
                        .or_default()
                        .push(record_index);
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

    pub(in crate::daemon::notifications::identity) fn rebuild_executable_index(&mut self) {
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
}
