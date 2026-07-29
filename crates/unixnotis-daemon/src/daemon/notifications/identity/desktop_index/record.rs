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
        let declared_executable_path = parsed_launch
            .as_ref()
            .map(|launch| launch.declared_path.clone());
        let declared_executable_identity = parsed_launch
            .as_ref()
            .map(|launch| launch.spec.declared_executable);
        let runtime_executable_path = parsed_launch
            .as_ref()
            .map(|launch| launch.runtime_path.clone());
        let runtime_executable_identity = parsed_launch
            .as_ref()
            .map(|launch| launch.spec.runtime_executable);
        let desktop_identity = executable_evidence_for_path(path).map(|evidence| evidence.identity);
        let launch_spec = parsed_launch.map(|launch| launch.spec);
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
            declared_executable_path,
            declared_executable_identity,
            runtime_executable_path,
            runtime_executable_identity,
            desktop_identity,
            desktop_provenance: InstallProvenance::Unknown,
            declared_executable_provenance: InstallProvenance::Unknown,
            runtime_executable_provenance: InstallProvenance::Unknown,
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
                    .chain(record.declared_executable_path.iter())
                    .chain(record.runtime_executable_path.iter())
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
            record.declared_executable_provenance = record
                .declared_executable_path
                .as_ref()
                .and_then(|path| ownership.get(path))
                .cloned()
                .unwrap_or(InstallProvenance::Unknown);
            record.runtime_executable_provenance = record
                .runtime_executable_path
                .as_ref()
                .and_then(|path| ownership.get(path))
                .cloned()
                .unwrap_or(InstallProvenance::Unknown);

            // A parsed target is promoted only when all protected files share one source
            if !runtime_binding_is_valid(record) {
                discard_untrusted_launcher_binding(record);
            }

            record.system_association = record.association_eligible
                && record
                    .desktop_identity
                    .is_some_and(super::super::executable::FileIdentity::is_system_managed)
                && record
                    .declared_executable_identity
                    .is_some_and(super::super::executable::FileIdentity::is_system_managed)
                && record
                    .runtime_executable_identity
                    .is_some_and(super::super::executable::FileIdentity::is_system_managed)
                && record
                    .launch_spec
                    .as_ref()
                    .is_some_and(|spec| spec.literal_files_are_system_managed)
                && record
                    .desktop_provenance
                    .same_application_source(&record.declared_executable_provenance)
                && record
                    .desktop_provenance
                    .same_application_source(&record.runtime_executable_provenance)
                && installed_identity_is_current(
                    record.declared_executable_path.as_deref(),
                    record.declared_executable_identity,
                )
                && installed_identity_is_current(
                    record.runtime_executable_path.as_deref(),
                    record.runtime_executable_identity,
                );
        }
        self.rebuild_executable_index();
        self.rebuild_application_families();
    }
}

fn discard_untrusted_launcher_binding(record: &mut DesktopRecord) {
    let Some(spec) = record.launch_spec.as_mut() else {
        return;
    };
    spec.package_launcher = None;

    // Falling back to the declared file preserves ordinary direct-executable behavior
    spec.runtime_executable = spec.declared_executable;
    record.runtime_executable_path = record.declared_executable_path.clone();
    record.runtime_executable_identity = record.declared_executable_identity;
    record.runtime_executable_provenance = record.declared_executable_provenance.clone();
}

fn runtime_binding_is_valid(record: &DesktopRecord) -> bool {
    let Some(spec) = record.launch_spec.as_ref() else {
        return false;
    };
    let direct_identity_matches = spec.declared_executable.same_file(spec.runtime_executable);
    let direct_path_matches = record.declared_executable_path == record.runtime_executable_path;
    let Some(binding) = spec.package_launcher.as_ref() else {
        return direct_identity_matches && direct_path_matches;
    };

    // Package equality is supporting evidence only after the literal file relationship exists
    binding
        .launcher_identity
        .same_file(spec.declared_executable)
        && binding.target_identity.same_file(spec.runtime_executable)
        && record.declared_executable_path.as_deref() == Some(&binding.launcher_path)
        && record.runtime_executable_path.as_deref() == Some(&binding.target_path)
        && record
            .desktop_provenance
            .same_application_source(&record.declared_executable_provenance)
        && record
            .desktop_provenance
            .same_application_source(&record.runtime_executable_provenance)
}

fn installed_identity_is_current(
    path: Option<&Path>,
    expected: Option<super::super::executable::FileIdentity>,
) -> bool {
    let (Some(path), Some(expected)) = (path, expected) else {
        return false;
    };
    executable_evidence_for_path(path).is_some_and(|current| {
        current.identity.same_file(expected)
            && current.identity.is_system_managed()
            && current.identity.is_executable_regular()
    })
}

#[cfg(test)]
#[path = "tests/record.rs"]
mod tests;

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
