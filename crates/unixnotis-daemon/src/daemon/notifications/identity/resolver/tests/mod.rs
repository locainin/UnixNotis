//! Resolver behavior tests grouped by evidence path

use std::collections::HashSet;
use std::path::PathBuf;

use unixnotis_core::{
    AttributionStatus, CommandLineQualityView, InlineReplyPolicy, InteractionPolicies,
    LaunchAuthorityView, LaunchVerificationView, RecordTrust,
};

use super::candidates::{resolve_unverified_candidates, strongest_verified_result};
use super::evidence::verify_record_sender;
use super::model::{CandidateVerification, SenderClaimRelation, VerifiedDesktopRecord};
use super::pipeline::{
    claim_has_index_candidate, needs_sender_provenance, resolve_attribution_owned_with,
    resolve_attribution_owned_with_pool, resolve_attribution_with_deadline, resolve_with_evidence,
    should_return_initial_resolution, ATTRIBUTION_TIMEOUT,
};
use super::sender_context::enrich_sender_install_provenance;
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
    refresh_sender_security_evidence, CommandLineEvidence, CommandLineQuality,
    ProcessLineageEvidence, SenderMetadata,
};
use crate::daemon::notifications::identity::FileIdentity;

mod support;

use support::*;

pub(super) async fn resolve_attribution(
    claim: AppClaim<'_>,
    sender: &SenderMetadata,
    index: &DesktopIdentityIndex,
) -> super::model::AttributionResolution {
    // Test-only direct entry point keeps asynchronous package enrichment covered
    let mut sender = refresh_sender_security_evidence(sender);
    let initial = resolve_with_evidence(claim, &sender, index);
    let needs_provenance = needs_sender_provenance(
        initial.attribution.status,
        initial.attribution.interactions,
        claim_has_index_candidate(claim, index),
    );
    if should_return_initial_resolution(needs_provenance) {
        return initial;
    }
    enrich_sender_install_provenance(&mut sender, index).await;
    resolve_with_evidence(claim, &sender, index)
}

mod candidates;
mod diagnostics;
mod evidence;
mod model;
mod pipeline;
mod resolution;
mod sender_context;
mod validation;
