//! Indexed desktop records and executable evidence

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use super::super::executable::FileIdentity;
use super::names::normalize_name;

#[derive(Debug, Clone)]
pub(in crate::daemon::notifications::identity) struct LaunchSpec {
    pub(in crate::daemon::notifications::identity) executable: FileIdentity,
    pub(in crate::daemon::notifications::identity) arguments: Vec<LaunchArgument>,
    pub(in crate::daemon::notifications::identity) protected_literal_files: usize,
    pub(in crate::daemon::notifications::identity) literal_files_are_system_managed: bool,
}

#[derive(Debug, Clone)]
pub(in crate::daemon::notifications::identity) enum LaunchArgument {
    Literal(LiteralArgument),
    FieldCode(FieldCode),
    OptionalIcon { name: String },
}

#[derive(Debug, Clone)]
pub(in crate::daemon::notifications::identity) struct LiteralArgument {
    pub(in crate::daemon::notifications::identity) value: Vec<u8>,
    pub(in crate::daemon::notifications::identity) file: Option<(PathBuf, FileIdentity)>,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(in crate::daemon::notifications::identity) enum FieldCode {
    File,
    Files,
    Url,
    Urls,
}

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
    pub(in crate::daemon::notifications::identity) launch_spec: Option<LaunchSpec>,
    pub(in crate::daemon::notifications::identity) names: HashSet<String>,
}

impl DesktopRecord {
    pub(in crate::daemon::notifications::identity) fn claim_matches(&self, claim: &str) -> bool {
        // Normalized aliases cover desktop names without trusting free-form display text
        self.names.contains(&normalize_name(claim))
    }
}

#[derive(Debug, Default)]
pub struct DesktopIdentityIndex {
    pub(super) records: Vec<DesktopRecord>,
    pub(super) by_id: HashMap<String, Vec<usize>>,
    pub(super) by_identity: HashMap<(u64, u64), Vec<usize>>,
    pub(super) system_brand_names: HashSet<String>,
    pub(in crate::daemon::notifications::identity) trusted_relays: Vec<ExecutableIdentity>,
    pub(in crate::daemon::notifications::identity) trusted_portals: Vec<ExecutableIdentity>,
}

#[derive(Debug, Clone)]
pub(in crate::daemon::notifications::identity) struct ExecutableIdentity {
    pub(in crate::daemon::notifications::identity) path: PathBuf,
    pub(in crate::daemon::notifications::identity) identity: FileIdentity,
}
