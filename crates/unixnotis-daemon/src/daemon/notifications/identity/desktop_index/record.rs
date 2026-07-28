//! Desktop-entry parsing and indexed record construction

use std::collections::HashSet;
use std::path::Path;

use gio::prelude::AppInfoExt;

use super::super::executable::{executable_evidence_for_path, FileIdentity};
use super::launch::build_launch_spec;
use super::model::{DesktopIdentityIndex, DesktopRecord};
use super::names::{normalize_desktop_id, normalize_name};

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
        let system_association = association_eligible
            && system_origin
            && desktop_identity.is_some_and(FileIdentity::is_system_managed)
            && executable_identity.is_some_and(FileIdentity::is_system_managed)
            && launch_spec
                .as_ref()
                .is_some_and(|spec| spec.literal_files_are_system_managed);
        let badge_icon = desktop
            .string("Icon")
            .map_or_else(|| id.clone(), |value| value.to_string());
        let names = association_aliases(&desktop, &id, &display_name, executable_path.as_deref());

        self.index_record(DesktopRecord {
            id,
            display_name,
            badge_icon,
            executable_path,
            executable_identity,
            desktop_identity,
            system_origin,
            system_association,
            association_eligible,
            dbus_activatable: desktop.boolean("DBusActivatable"),
            launch_spec,
            names,
        });
    }
}

fn association_aliases(
    desktop: &gio::DesktopAppInfo,
    id: &str,
    display_name: &str,
    executable_path: Option<&Path>,
) -> HashSet<String> {
    // These aliases are considered only after executable identity already agrees
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
    if let Some(executable) = executable_path
        .and_then(Path::file_name)
        .and_then(|name| name.to_str())
    {
        names.insert(normalize_name(executable));
    }
    names.retain(|name| !name.is_empty());
    names
}
