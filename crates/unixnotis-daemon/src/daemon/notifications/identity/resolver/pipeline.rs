//! Ordered attribution pipeline and candidate orchestration

use std::sync::{Arc, OnceLock};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use unixnotis_core::{AttributionStatus, InteractionPolicies, RecordTrust};

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
    trusted_portal_path, unknown_reply_denied,
};
use super::sender_context::enrich_sender_install_provenance_blocking;
use super::validation::validate_desktop_id;

const ATTRIBUTION_WORKER_SLOTS: usize = 8;

fn attribution_worker_pool() -> Arc<Semaphore> {
    static POOL: OnceLock<Arc<Semaphore>> = OnceLock::new();
    Arc::clone(POOL.get_or_init(|| Arc::new(Semaphore::new(ATTRIBUTION_WORKER_SLOTS))))
}

fn try_attribution_worker() -> Option<OwnedSemaphorePermit> {
    attribution_worker_pool().try_acquire_owned().ok()
}

/// Production entry point that moves procfs and filesystem work off Tokio workers
pub(in crate::daemon) async fn resolve_attribution_owned(
    reported_name: String,
    desktop_entry: Option<String>,
    sender: SenderMetadata,
    index: Arc<DesktopIdentityIndex>,
) -> AttributionResolution {
    let Some(initial_permit) = try_attribution_worker() else {
        let claim = AppClaim {
            reported_name: &reported_name,
            desktop_entry: desktop_entry.as_deref(),
        };
        return unknown_reply_denied(claim, &sender, "attribution worker capacity exhausted");
    };
    let fallback_sender = sender.clone();
    let result = tokio::time::timeout(
        std::time::Duration::from_millis(500),
        tokio::task::spawn_blocking({
            let reported_name = reported_name.clone();
            let desktop_entry = desktop_entry.clone();
            let index = Arc::clone(&index);
            let sender = sender.clone();
            move || {
                // The permit lives inside the blocking closure so timeout cancellation
                // cannot release capacity while procfs work is still running
                let _permit = initial_permit;
                let sender = refresh_sender_security_evidence(&sender);
                let claim = AppClaim {
                    reported_name: &reported_name,
                    desktop_entry: desktop_entry.as_deref(),
                };
                let resolution = resolve_with_evidence(claim, &sender, &index);
                let needs = needs_sender_provenance(
                    resolution.attribution.status,
                    resolution.attribution.interactions,
                    claim_has_index_candidate(claim, &index),
                );
                if !needs {
                    return resolution;
                }

                let mut sender = sender;
                enrich_sender_install_provenance_blocking(&mut sender, &index);
                resolve_with_evidence(claim, &sender, &index)
            }
        }),
    )
    .await;
    let resolution = match result {
        Ok(Ok(resolution)) => resolution,
        Ok(Err(_)) => {
            let claim = AppClaim {
                reported_name: &reported_name,
                desktop_entry: desktop_entry.as_deref(),
            };
            return unknown_reply_denied(claim, &fallback_sender, "attribution worker stopped");
        }
        Err(_) => {
            let claim = AppClaim {
                reported_name: &reported_name,
                desktop_entry: desktop_entry.as_deref(),
            };
            return unknown_reply_denied(claim, &fallback_sender, "attribution timed out");
        }
    };
    resolution
}

#[cfg_attr(
    not(test),
    expect(dead_code, reason = "helper remains as an explicit pipeline test seam")
)]
pub(super) const fn should_return_initial_resolution(needs_provenance: bool) -> bool {
    !needs_provenance
}

pub(super) fn needs_sender_provenance(
    status: AttributionStatus,
    interactions: InteractionPolicies,
    claim_has_candidate: bool,
) -> bool {
    // Package lookup is useful only while it can positively bind a denied helper
    interactions == InteractionPolicies::DENY
        && (status == AttributionStatus::Recognized
            || (status == AttributionStatus::Unresolved && claim_has_candidate))
}

pub(super) fn claim_has_index_candidate(claim: AppClaim<'_>, index: &DesktopIdentityIndex) -> bool {
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
        // A trusted portal executable associates branding but cannot authenticate the origin
        let record = preferred_record(&hint_records);
        if !claim.reported_name.trim().is_empty()
            && !index.record_matches_claim(record, claim.reported_name)
        {
            // A portal desktop id and a different caller label remain contradictory evidence
            let mut resolution = conflict_from_candidate(
                claim,
                sender,
                index,
                record,
                LaunchFailure::DesktopClaimMismatch,
            );
            resolution.diagnostics.record_trust = RecordTrust::Portal;
            resolution.diagnostics.reason =
                "portal application id contradicted the reported name".to_string();
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
        resolution.diagnostics.reason = "portal-mediated application association".to_string();
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
