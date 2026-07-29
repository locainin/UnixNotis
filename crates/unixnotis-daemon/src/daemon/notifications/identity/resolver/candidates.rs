//! Candidate filtering, ranking, and ambiguity handling

use std::collections::HashSet;

use unixnotis_core::{AttributionReason, AttributionStatus, NotificationAttribution};

use super::super::desktop_index::{
    normalize_desktop_id, normalize_name, DesktopIdentityIndex, DesktopRecord, LaunchFailure,
    LaunchVerification, VerifiedLaunch,
};
use super::super::sender::SenderMetadata;
use super::diagnostics::{launch_failure_label, with_diagnostics};
use super::evidence::{candidate_proves_conflict, lineage_association, sender_claim_relation};
use super::model::{CandidateVerification, SenderClaimRelation, VerifiedDesktopRecord};
use super::resolution::{
    conflict_from_candidate, policy_resolution, recognized_resolution, sender_claim_group_key,
};
use super::{AppClaim, AttributionResolution};

pub(super) fn resolve_unverified_candidates(
    claim: AppClaim<'_>,
    sender: &SenderMetadata,
    index: &DesktopIdentityIndex,
    hint_records: &[&DesktopRecord],
    results: &[CandidateVerification<'_>],
) -> AttributionResolution {
    let matching_results = results
        .iter()
        .filter(|result| {
            index.record_matches_claim(result.record, claim.reported_name)
                || (claim.reported_name.trim().is_empty()
                    && hint_records
                        .iter()
                        .any(|record| std::ptr::eq(*record, result.record)))
        })
        .collect::<Vec<_>>();

    // A same-user ancestor can explain a helper without authenticating helper-owned actions
    if let Some((record, detail)) = lineage_association(sender, index, &matching_results) {
        let failure = matching_results
            .iter()
            .find(|result| std::ptr::eq(result.record, record))
            .map_or(LaunchFailure::ExecutableMismatch, |result| result.failure());
        return with_diagnostics(
            recognized_resolution(claim, sender, record, index, failure, &detail),
            claim,
            sender,
            Some(record),
            LaunchVerification::InsufficientEvidence(failure),
        );
    }

    // Only protected records can turn a caller-provided label into a conflict
    let protected_mismatches = matching_results
        .iter()
        .copied()
        .filter(|result| {
            result.is_definitive_mismatch()
                && result.record.system_origin
                && result.record.system_association
                && candidate_proves_conflict(sender, index, result)
        })
        .collect::<Vec<_>>();
    if let Some(first) = protected_mismatches.first().copied() {
        // Distinct protected families with the same label are ambiguous, not suspicious
        if !protected_mismatches
            .iter()
            .all(|candidate| index.records_share_family(first.record, candidate.record))
        {
            let detail = "Multiple protected desktop application families matched the claim";
            return with_diagnostics(
                policy_resolution(NotificationAttribution::unresolved(
                    claim.reported_name,
                    AttributionReason::AmbiguousDesktopRecords,
                    detail,
                    sender_claim_group_key(
                        AttributionStatus::Unresolved,
                        claim.reported_name,
                        sender,
                    ),
                )),
                claim,
                sender,
                None,
                LaunchVerification::InsufficientEvidence(
                    LaunchFailure::AmbiguousDesktopAssociation,
                ),
            );
        }
        let mismatch = protected_mismatches
            .into_iter()
            .max_by_key(|candidate| {
                normalize_desktop_id(&candidate.record.id)
                    == normalize_desktop_id(index.canonical_id_for_record(candidate.record))
            })
            .unwrap_or(first);
        return conflict_from_candidate(claim, sender, index, mismatch.record, mismatch.failure());
    }

    if let Some(resolution) =
        ambiguous_protected_family_resolution(claim, sender, index, &matching_results)
    {
        return resolution;
    }

    // A known application with incomplete evidence remains useful but non-authoritative
    if let Some(candidate) = matching_results
        .iter()
        .max_by_key(|result| record_trust_rank(result.record))
    {
        let failure = candidate.failure();
        let detail = recognized_candidate_detail(sender, index, candidate.record, failure);
        return with_diagnostics(
            recognized_resolution(claim, sender, candidate.record, index, failure, &detail),
            claim,
            sender,
            Some(candidate.record),
            candidate.verification,
        );
    }

    unresolved_candidate_resolution(claim, sender, index)
}

fn recognized_candidate_detail(
    sender: &SenderMetadata,
    index: &DesktopIdentityIndex,
    record: &DesktopRecord,
    failure: LaunchFailure,
) -> String {
    match sender_claim_relation(sender, index, record) {
        SenderClaimRelation::SamePackageHelper => {
            "Sender belongs to the same installed application package but was not strongly bound"
                .to_string()
        }
        SenderClaimRelation::DifferentInstalledPackage => {
            "Sender belongs to a separate installed package without a conflicting application identity"
                .to_string()
        }
        SenderClaimRelation::ClaimedApplication
        | SenderClaimRelation::DifferentVerifiedApplication
        | SenderClaimRelation::UnknownExecutable
        | SenderClaimRelation::TrustedRelay => {
            launch_failure_label(failure).to_string()
        }
    }
}

fn ambiguous_protected_family_resolution(
    claim: AppClaim<'_>,
    sender: &SenderMetadata,
    index: &DesktopIdentityIndex,
    matching_results: &[&CandidateVerification<'_>],
) -> Option<AttributionResolution> {
    let protected_families = matching_results
        .iter()
        .filter(|result| result.record.system_association)
        .filter_map(|result| index.family_index_for_record(result.record))
        .collect::<HashSet<_>>();
    if protected_families.len() <= 1 {
        return None;
    }

    let detail = "Multiple protected desktop application families matched the claim";
    Some(with_diagnostics(
        policy_resolution(NotificationAttribution::unresolved(
            claim.reported_name,
            AttributionReason::AmbiguousDesktopRecords,
            detail,
            sender_claim_group_key(AttributionStatus::Unresolved, claim.reported_name, sender),
        )),
        claim,
        sender,
        None,
        LaunchVerification::InsufficientEvidence(LaunchFailure::AmbiguousDesktopAssociation),
    ))
}

fn unresolved_candidate_resolution(
    claim: AppClaim<'_>,
    sender: &SenderMetadata,
    index: &DesktopIdentityIndex,
) -> AttributionResolution {
    let reason = if index.claim_matches_system_app(claim.reported_name) {
        AttributionReason::NoDesktopCandidate
    } else if sender.sender_executable_identity.is_none() {
        AttributionReason::MissingSenderEvidence
    } else {
        AttributionReason::NoDesktopCandidate
    };
    let detail = sender.sender_executable.as_deref().map_or_else(
        || "No reliable desktop application candidate was found".to_string(),
        |path| format!("No desktop application matched source {path}"),
    );
    with_diagnostics(
        policy_resolution(NotificationAttribution::unresolved(
            claim.reported_name,
            reason,
            &detail,
            sender_claim_group_key(AttributionStatus::Unresolved, claim.reported_name, sender),
        )),
        claim,
        sender,
        None,
        LaunchVerification::InsufficientEvidence(LaunchFailure::NoDesktopCandidate),
    )
}

pub(super) fn trusted_relay_resolution(
    claim: AppClaim<'_>,
    sender: &SenderMetadata,
    index: &DesktopIdentityIndex,
) -> Option<AttributionResolution> {
    let identity = sender.sender_executable_identity?;
    let path = index.trusted_relay_path(identity)?;
    let group_key = format!(
        "relay:{}:{}",
        identity.group_fragment(),
        normalize_name(claim.reported_name)
    );
    let attribution = NotificationAttribution::relay(
        claim.reported_name,
        &format!("Sent through {}", path.display()),
        group_key,
    );
    let mut resolution = with_diagnostics(
        policy_resolution(attribution),
        claim,
        sender,
        None,
        LaunchVerification::Verified(VerifiedLaunch::DedicatedExecutable),
    );
    resolution.diagnostics.reason = "verified trusted relay executable".to_string();
    Some(resolution)
}

pub(super) fn strongest_verified_result<'record>(
    results: &[CandidateVerification<'record>],
    reported_name: &str,
    index: &DesktopIdentityIndex,
) -> Option<VerifiedDesktopRecord<'record>> {
    let missing_name = reported_name.trim().is_empty();
    // Rank every verified candidate before deciding whether the strongest tier is ambiguous
    let verified = results
        .iter()
        .filter(|result| {
            matches!(result.verification, LaunchVerification::Verified(_))
                && (missing_name || index.record_matches_claim(result.record, reported_name))
        })
        .collect::<Vec<_>>();
    let maximum_rank = verified
        .iter()
        .map(|result| record_trust_rank(result.record))
        .max()?;
    let strongest = verified
        .into_iter()
        .filter(|result| record_trust_rank(result.record) == maximum_rank)
        .collect::<Vec<_>>();
    let families = strongest
        .iter()
        .filter_map(|result| index.family_index_for_record(result.record))
        .collect::<HashSet<_>>();
    // maximum_rank guarantees at least one strongest candidate
    if families.len() != 1 {
        return None;
    }
    let preferred = strongest.into_iter().min_by_key(|candidate| {
        let canonical = index.canonical_id_for_record(candidate.record);
        let normalized_id = normalize_desktop_id(&candidate.record.id);
        let is_alias = normalized_id != normalize_desktop_id(canonical);
        (is_alias, normalized_id)
    })?;
    let LaunchVerification::Verified(launch) = preferred.verification else {
        return None;
    };
    Some(VerifiedDesktopRecord(preferred.record, launch))
}

pub(super) const fn record_trust_rank(record: &DesktopRecord) -> u8 {
    if record.system_association {
        2
    } else {
        1
    }
}

pub(super) fn preferred_record<'record>(
    records: &[&'record DesktopRecord],
) -> &'record DesktopRecord {
    records
        .iter()
        .copied()
        .max_by_key(|record| record_trust_rank(record))
        .expect("caller checks that a desktop candidate exists")
}

pub(super) fn extend_unique_records<'record>(
    records: &mut Vec<&'record DesktopRecord>,
    additions: Vec<&'record DesktopRecord>,
) {
    for record in additions {
        if !records
            .iter()
            .any(|existing| std::ptr::eq(*existing, record))
        {
            records.push(record);
        }
    }
}
