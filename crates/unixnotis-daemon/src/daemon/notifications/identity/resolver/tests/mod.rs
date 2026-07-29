//! Resolver behavior tests grouped by evidence path

use std::collections::HashSet;
use std::path::PathBuf;

use unixnotis_core::{
    AttributionStatus, CommandLineQualityView, InlineReplyPolicy, LaunchAuthorityView,
    LaunchVerificationView, RecordTrust,
};

use super::candidates::{resolve_unverified_candidates, strongest_verified_result};
use super::evidence::verify_record_sender;
use super::model::{CandidateVerification, SenderClaimRelation, VerifiedDesktopRecord};
use super::pipeline::{resolve_attribution, resolve_with_evidence};
use super::AppClaim;
use crate::daemon::notifications::identity::desktop_index::model::{
    ExecutableIdentity, FieldCode, LaunchArgument, LaunchSpec, LiteralArgument,
};
use crate::daemon::notifications::identity::desktop_index::provenance::PackageProvider;
use crate::daemon::notifications::identity::desktop_index::{
    normalize_name, DesktopIdentityIndex, DesktopRecord, InstallProvenance, LaunchFailure,
    LaunchVerification, VerifiedLaunch,
};
use crate::daemon::notifications::identity::executable::executable_evidence_for_path;
use crate::daemon::notifications::identity::sender::{
    CommandLineEvidence, CommandLineQuality, ProcessLineageEvidence, SenderMetadata,
};
use crate::daemon::notifications::identity::FileIdentity;

mod support;

use support::*;

mod candidates;
mod diagnostics;
mod evidence;
mod model;
mod pipeline;
mod resolution;
mod sender_context;
mod validation;
