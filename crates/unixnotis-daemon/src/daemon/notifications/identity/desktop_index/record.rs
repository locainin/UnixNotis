//! Desktop-entry parsing and indexed record construction

use std::collections::HashSet;
use std::path::Path;

use gio::prelude::AppInfoExt;

use super::super::executable::executable_evidence_for_path;
use super::launch::build_launch_spec;
use super::model::{DesktopIdentityIndex, DesktopRecord};
use super::names::{normalize_desktop_id, normalize_name};
use super::provenance::InstallProvenance;

impl DesktopIdentityIndex {
    pub(super) fn add_desktop_file(&mut self, path: &Path, system_origin: bool) {
        // GIO applies desktop-entry parsing rules before any identity is indexed
        let Some(desktop) = gio::DesktopAppInfo::from_filename(path) else {
            return;
        };
        let Some(id) = desktop
            .id()
            .map(|value| normalize_desktop_id(value.as_str()))
        else {
            return;
        };
        if id.is_empty() {
            return;
        }
        let display_name = desktop.display_name().to_string();
        // Wrapper normalization finds the application executable instead of indexing env itself
        let parsed_launch = build_launch_spec(&desktop, path);
        let executable_path = parsed_launch
            .as_ref()
            .map(|(executable_path, _spec)| executable_path.clone());
        let executable_identity = parsed_launch
            .as_ref()
            .map(|(_executable_path, spec)| spec.executable);
        let desktop_identity = executable_evidence_for_path(path).map(|evidence| evidence.identity);
        let launch_spec = parsed_launch.map(|(_executable_path, spec)| spec);
        // Every association needs a complete Exec contract instead of a runtime-name exception
        let association_eligible = launch_spec.is_some();
        // System association requires protected metadata and a reproducible launch specification
        // Package ownership is attached in one bounded batch after scanning finishes
        let system_association = false;
        let badge_icon = desktop
            .string("Icon")
            .map_or_else(|| id.clone(), |value| value.to_string());
        let names = association_aliases(&desktop, &id, &display_name);

        self.index_record(DesktopRecord {
            id,
            display_name,
            badge_icon,
            desktop_path: Some(path.to_path_buf()),
            executable_path,
            executable_identity,
            desktop_identity,
            desktop_provenance: InstallProvenance::Unknown,
            executable_provenance: InstallProvenance::Unknown,
            system_origin,
            system_association,
            association_eligible,
            launch_spec,
            names,
        });
    }

    pub(super) fn finalize_install_provenance(&mut self) {
        let paths = self
            .records
            .iter()
            .filter(|record| record.system_origin)
            .flat_map(|record| {
                record
                    .desktop_path
                    .iter()
                    .chain(record.executable_path.iter())
                    .cloned()
            })
            .collect::<Vec<_>>();
        let ownership = self.package_ownership.resolve_many(paths);

        for record in &mut self.records {
            if !record.system_origin {
                continue;
            }
            record.desktop_provenance = record
                .desktop_path
                .as_ref()
                .and_then(|path| ownership.get(path))
                .cloned()
                .unwrap_or(InstallProvenance::Unknown);
            record.executable_provenance = record
                .executable_path
                .as_ref()
                .and_then(|path| ownership.get(path))
                .cloned()
                .unwrap_or(InstallProvenance::Unknown);
            record.system_association = record.association_eligible
                && record
                    .desktop_identity
                    .is_some_and(super::super::executable::FileIdentity::is_system_managed)
                && record
                    .executable_identity
                    .is_some_and(super::super::executable::FileIdentity::is_system_managed)
                && record
                    .launch_spec
                    .as_ref()
                    .is_some_and(|spec| spec.literal_files_are_system_managed)
                && record
                    .desktop_provenance
                    .same_application_source(&record.executable_provenance);
        }
        self.rebuild_application_families();
    }
}

fn association_aliases(
    desktop: &gio::DesktopAppInfo,
    id: &str,
    display_name: &str,
) -> HashSet<String> {
    // Desktop metadata supplies claim aliases while executable naming stays separate
    let mut names = HashSet::from([
        normalize_name(display_name),
        normalize_name(desktop.name().as_str()),
        normalize_name(id),
    ]);
    if let Some(generic_name) = desktop.generic_name() {
        names.insert(normalize_name(generic_name.as_str()));
    }
    if let Some(wm_class) = desktop.startup_wm_class() {
        names.insert(normalize_name(wm_class.as_str()));
    }
    names.retain(|name| !name.is_empty());
    names
}
