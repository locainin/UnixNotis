//! Ordered attribution pipeline and candidate orchestration

use unixnotis_core::{AttributionStatus, RecordTrust};

use super::super::desktop_index::{
    DesktopIdentityIndex, LaunchFailure, LaunchVerification, VerifiedLaunch,
};
use super::super::sender::{refresh_sender_security_evidence, SenderMetadata};
use super::candidates::{
    extend_unique_records, preferred_record, resolve_unverified_candidates,
    strongest_verified_result, trusted_relay_resolution,
};
use super::diagnostics::with_diagnostics;
use super::evidence::verify_record_sender;
use super::model::{AppClaim, AttributionResolution, CandidateVerification};
use super::resolution::{
    conflict_from_candidate, resolution_for_portal_record, resolution_for_record,
    trusted_portal_path,
};
use super::sender_context::enrich_sender_install_provenance;
use super::validation::validate_desktop_id;

pub(in crate::daemon) async fn resolve_attribution(
    claim: AppClaim<'_>,
    sender: &SenderMetadata,
    index: &DesktopIdentityIndex,
) -> AttributionResolution {
    // Cached process data is refreshed before it affects attribution
    let mut sender = refresh_sender_security_evidence(sender);
    let initial = resolve_with_evidence(claim, &sender, index);
    if initial.attribution.status != AttributionStatus::Recognized
        && !(initial.attribution.status == AttributionStatus::Unresolved
            && claim_has_index_candidate(claim, index))
    {
        return initial;
    }

    // Ownership is needed only to distinguish a probable helper from a different installed app
    enrich_sender_install_provenance(&mut sender, index).await;
    resolve_with_evidence(claim, &sender, index)
}

fn claim_has_index_candidate(claim: AppClaim<'_>, index: &DesktopIdentityIndex) -> bool {
    if !claim.reported_name.trim().is_empty()
        && !index.records_for_claim(claim.reported_name).is_empty()
    {
        return true;
    }

    claim
        .desktop_entry
        .and_then(validate_desktop_id)
        .is_some_and(|desktop_id| !index.records_for_id(&desktop_id).is_empty())
}

pub(super) fn resolve_with_evidence(
    claim: AppClaim<'_>,
    sender: &SenderMetadata,
    index: &DesktopIdentityIndex,
) -> AttributionResolution {
    let desktop_entry = claim.desktop_entry.and_then(validate_desktop_id);
    let hint_records = desktop_entry
        .as_deref()
        .map_or_else(Vec::new, |desktop_id| index.records_for_id(desktop_id));
    if desktop_entry.is_some()
        && !hint_records.is_empty()
        && trusted_portal_path(sender, index).is_some()
    {
        // A trusted portal executable may forward its broker-owned application id
        let record = preferred_record(&hint_records);
        if !claim.reported_name.trim().is_empty()
            && !index.record_matches_claim(record, claim.reported_name)
        {
            // A protected portal id and a different caller label are affirmative contradiction
            let mut resolution = conflict_from_candidate(
                claim,
                sender,
                index,
                record,
                LaunchFailure::DesktopClaimMismatch,
            );
            resolution.diagnostics.record_trust = RecordTrust::Portal;
            resolution.diagnostics.reason =
                "verified portal application id contradicted the reported name".to_string();
            return resolution;
        }
        let mut resolution = with_diagnostics(
            resolution_for_portal_record(record, claim.reported_name, sender, index),
            claim,
            sender,
            Some(record),
            LaunchVerification::Verified(VerifiedLaunch::DedicatedExecutable),
        );
        resolution.diagnostics.record_trust = RecordTrust::Portal;
        resolution.diagnostics.reason = "verified portal application identity".to_string();
        return resolution;
    }

    // Hints, executable identity, and claimed names contribute candidates without granting trust
    let mut candidates = hint_records.clone();
    if let Some(identity) = sender.sender_executable_identity {
        extend_unique_records(&mut candidates, index.records_for_executable(identity));
    }
    if !claim.reported_name.trim().is_empty() {
        extend_unique_records(
            &mut candidates,
            index.records_for_claim(claim.reported_name),
        );
    }
    let results = candidates
        .iter()
        .map(|record| CandidateVerification {
            record,
            verification: verify_record_sender(record, sender, index),
        })
        .collect::<Vec<_>>();

    if let Some(record) = strongest_verified_result(&results, claim.reported_name, index) {
        return with_diagnostics(
            resolution_for_record(record, claim.reported_name, sender, index),
            claim,
            sender,
            Some(record.0),
            LaunchVerification::Verified(record.1),
        );
    }

    // A verified relay identifies itself but never authenticates the forwarded label
    if let Some(resolution) = trusted_relay_resolution(claim, sender, index) {
        return resolution;
    }

    resolve_unverified_candidates(claim, sender, index, &hint_records, &results)
}
