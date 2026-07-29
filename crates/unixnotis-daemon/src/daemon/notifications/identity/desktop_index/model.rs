//! Indexed desktop records and executable evidence

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use super::super::executable::FileIdentity;
use super::names::normalize_name;
use super::provenance::{InstallProvenance, PackageOwnershipCache};

#[derive(Debug, Clone)]
pub(in crate::daemon::notifications::identity) struct LaunchSpec {
    pub(in crate::daemon::notifications::identity) executable: FileIdentity,
    pub(in crate::daemon::notifications::identity) arguments: Vec<LaunchArgument>,
    pub(in crate::daemon::notifications::identity) environment: Vec<(Vec<u8>, Vec<u8>)>,
    pub(in crate::daemon::notifications::identity) wrappers: Vec<LaunchWrapper>,
    pub(in crate::daemon::notifications::identity) literal_files_are_system_managed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::daemon::notifications::identity) enum LaunchArgument {
    Literal(LiteralArgument),
    FieldCode(FieldCode),
    OptionalIcon { name: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
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

/// Wrapper programs removed before application identity is evaluated
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(in crate::daemon::notifications::identity) enum LaunchWrapper {
    Env,
}

/// Evidence that establishes which application a desktop record launches
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(in crate::daemon::notifications::identity) enum LaunchAuthority {
    DedicatedExecutable,
    ProtectedPayload,
    DynamicOnly,
    Ambiguous,
}

/// Positive launch identity retained for diagnostics and candidate ranking
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(in crate::daemon::notifications::identity) enum VerifiedLaunch {
    DedicatedExecutable,
    ProtectedPayload,
}

/// Stable reason for a launch decision that cannot authenticate the claim
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(in crate::daemon::notifications::identity) enum LaunchFailure {
    MissingSenderEvidence,
    MissingCommandLine,
    UnstructuredCommandLine,
    EmptyContractNeedsCommandLine,
    UnsupportedWrapper,
    AmbiguousDesktopAssociation,
    DynamicOnlyContract,
    ExecutableMismatch,
    ProtectedPayloadMismatch,
    RequiredArgumentMismatch,
    DesktopClaimMismatch,
    NoDesktopCandidate,
}

/// Three-way launch result keeps missing evidence distinct from contradiction
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(in crate::daemon::notifications::identity) enum LaunchVerification {
    Verified(VerifiedLaunch),
    InsufficientEvidence(LaunchFailure),
    DefinitiveMismatch(LaunchFailure),
}

#[derive(Debug, Clone)]
pub(in crate::daemon::notifications::identity) struct DesktopRecord {
    pub(in crate::daemon::notifications::identity) id: String,
    pub(in crate::daemon::notifications::identity) display_name: String,
    pub(in crate::daemon::notifications::identity) badge_icon: String,
    pub(in crate::daemon::notifications::identity) desktop_path: Option<PathBuf>,
    pub(in crate::daemon::notifications::identity) executable_path: Option<PathBuf>,
    pub(in crate::daemon::notifications::identity) executable_identity: Option<FileIdentity>,
    pub(in crate::daemon::notifications::identity) desktop_identity: Option<FileIdentity>,
    pub(in crate::daemon::notifications::identity) desktop_provenance: InstallProvenance,
    pub(in crate::daemon::notifications::identity) executable_provenance: InstallProvenance,
    pub(in crate::daemon::notifications::identity) system_origin: bool,
    pub(in crate::daemon::notifications::identity) system_association: bool,
    pub(in crate::daemon::notifications::identity) association_eligible: bool,
    pub(in crate::daemon::notifications::identity) launch_spec: Option<LaunchSpec>,
    pub(in crate::daemon::notifications::identity) names: HashSet<String>,
}

impl DesktopRecord {
    pub(in crate::daemon::notifications::identity) fn claim_matches(&self, claim: &str) -> bool {
        // Normalized aliases cover desktop names without trusting free-form display text
        self.names.contains(&normalize_name(claim))
    }
}

/// Canonical identity shared by equivalent desktop-entry aliases
#[derive(Debug, Clone)]
pub(in crate::daemon::notifications::identity) struct DesktopApplicationFamily {
    pub(in crate::daemon::notifications::identity) canonical_id: String,
    pub(in crate::daemon::notifications::identity) executable_identity: FileIdentity,
    pub(in crate::daemon::notifications::identity) records: Vec<usize>,
    pub(in crate::daemon::notifications::identity) names: HashSet<String>,
    pub(in crate::daemon::notifications::identity) system_origin: bool,
    pub(in crate::daemon::notifications::identity) system_association: bool,
    pub(in crate::daemon::notifications::identity) install_provenance: InstallProvenance,
    pub(in crate::daemon::notifications::identity) protected_payloads: Vec<(usize, u64, u64)>,
}

#[derive(Debug, Default)]
pub struct DesktopIdentityIndex {
    pub(super) records: Vec<DesktopRecord>,
    pub(super) families: Vec<DesktopApplicationFamily>,
    pub(super) family_by_record: Vec<Option<usize>>,
    pub(super) by_id: HashMap<String, Vec<usize>>,
    pub(super) by_identity: HashMap<(u64, u64), Vec<usize>>,
    pub(super) by_name: HashMap<String, Vec<usize>>,
    pub(super) system_brand_names: HashSet<String>,
    pub(in crate::daemon::notifications::identity) trusted_relays: Vec<ExecutableIdentity>,
    pub(in crate::daemon::notifications::identity) trusted_portals: Vec<ExecutableIdentity>,
    pub(super) package_ownership: Arc<PackageOwnershipCache>,
}

#[derive(Debug, Clone)]
pub(in crate::daemon::notifications::identity) struct ExecutableIdentity {
    pub(in crate::daemon::notifications::identity) path: PathBuf,
    pub(in crate::daemon::notifications::identity) identity: FileIdentity,
}
