//! Indexed desktop records and executable evidence

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use super::super::executable::FileIdentity;
use super::names::normalize_name;

#[derive(Debug, Clone)]
pub(in crate::daemon::notifications::identity) struct DesktopRecord {
    pub(in crate::daemon::notifications::identity) id: String,
    pub(in crate::daemon::notifications::identity) display_name: String,
    pub(in crate::daemon::notifications::identity) badge_icon: String,
    pub(in crate::daemon::notifications::identity) executable_path: Option<PathBuf>,
    pub(in crate::daemon::notifications::identity) executable_identity: Option<FileIdentity>,
    pub(in crate::daemon::notifications::identity) desktop_identity: Option<FileIdentity>,
    pub(in crate::daemon::notifications::identity) system_origin: bool,
    pub(in crate::daemon::notifications::identity) system_association: bool,
    pub(in crate::daemon::notifications::identity) association_eligible: bool,
    pub(in crate::daemon::notifications::identity) dbus_activatable: bool,
    pub(super) names: HashSet<String>,
}

impl DesktopRecord {
    pub(in crate::daemon::notifications::identity) fn claim_matches(&self, claim: &str) -> bool {
        // Normalized aliases cover desktop names without trusting free-form display text
        self.names.contains(&normalize_name(claim))
    }

    #[cfg(test)]
    pub(in crate::daemon::notifications::identity) fn fixture(
        id: &str,
        display_name: &str,
        executable_path: &str,
        identity: FileIdentity,
        system_entry: bool,
        dbus_activatable: bool,
    ) -> Self {
        let names = HashSet::from([normalize_name(display_name)]);
        Self {
            id: id.to_string(),
            display_name: display_name.to_string(),
            badge_icon: id.to_string(),
            executable_path: Some(PathBuf::from(executable_path)),
            executable_identity: Some(identity),
            desktop_identity: Some(identity),
            system_origin: system_entry,
            system_association: system_entry,
            association_eligible: true,
            dbus_activatable,
            names,
        }
    }
}

#[derive(Debug, Default)]
pub(in crate::daemon) struct DesktopIdentityIndex {
    pub(super) records: Vec<DesktopRecord>,
    pub(super) by_id: HashMap<String, Vec<usize>>,
    pub(super) by_identity: HashMap<(u64, u64), Vec<usize>>,
    pub(super) system_brand_names: HashSet<String>,
    pub(super) trusted_relays: Vec<ExecutableIdentity>,
}

#[derive(Debug, Clone)]
pub(super) struct ExecutableIdentity {
    pub(super) path: PathBuf,
    pub(super) identity: FileIdentity,
}
