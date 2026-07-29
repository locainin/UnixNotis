//! Ordered application attribution from process, portal, and desktop evidence

use std::collections::HashSet;

use unixnotis_core::{
    AttributionDiagnostics, AttributionReason, AttributionStatus, InlineReplyPolicy,
    NotificationAttribution, RecordTrust,
};
use zbus::fdo::DBusProxy;
use zbus::Connection;

use super::desktop_index::{
    normalize_desktop_id, normalize_name, verify_record_launch, DesktopIdentityIndex,
    DesktopRecord, InstallProvenance, LaunchFailure, LaunchVerification, VerifiedLaunch,
};
use super::executable::{executable_evidence_for_path, FileIdentity};
use super::policy::inline_reply_policy;
use super::sender::{refresh_sender_security_evidence, CommandLineEvidence, SenderMetadata};

mod candidates;
mod diagnostics;
mod evidence;
mod resolution;

use candidates::{
    extend_unique_records, preferred_record, resolve_unverified_candidates,
    strongest_verified_result, trusted_relay_resolution,
};
use diagnostics::with_diagnostics;
use evidence::{current_system_identity_matches_sender, verify_record_sender};
use resolution::{
    policy_resolution, resolution_for_portal_record, resolution_for_record, sender_claim_group_key,
    trusted_portal_path,
};

const MAX_DESKTOP_ID_BYTES: usize = 256;

#[derive(Clone, Copy)]
pub(in crate::daemon) struct AppClaim<'a> {
    pub(in crate::daemon) reported_name: &'a str,
    pub(in crate::daemon) desktop_entry: Option<&'a str>,
}

pub(in crate::daemon) struct AttributionResolution {
    pub(in crate::daemon) attribution: NotificationAttribution,
    pub(in crate::daemon) diagnostics: AttributionDiagnostics,
    pub(in crate::daemon) inline_reply_policy: InlineReplyPolicy,
}

#[derive(Clone, Copy)]
struct VerifiedDesktopRecord<'record>(&'record DesktopRecord, VerifiedLaunch);

#[derive(Clone, Copy)]
struct CandidateVerification<'record> {
    record: &'record DesktopRecord,
    verification: LaunchVerification,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum SenderClaimRelation {
    ClaimedApplication,
    DifferentVerifiedApplication,
    SamePackageHelper,
    UnknownExecutable,
    TrustedRelay,
}

impl CandidateVerification<'_> {
    const fn is_definitive_mismatch(&self) -> bool {
        matches!(self.verification, LaunchVerification::DefinitiveMismatch(_))
    }

    const fn failure(&self) -> LaunchFailure {
        match self.verification {
            LaunchVerification::Verified(_) => LaunchFailure::DesktopClaimMismatch,
            LaunchVerification::InsufficientEvidence(reason)
            | LaunchVerification::DefinitiveMismatch(reason) => reason,
        }
    }
}

pub(in crate::daemon) fn unknown_reply_denied(
    claim: AppClaim<'_>,
    sender: &SenderMetadata,
    reason: &str,
) -> AttributionResolution {
    let detail = sender.sender_executable.as_deref().map_or_else(
        || reason.to_string(),
        |path| format!("{reason}; source {path}"),
    );
    let resolution = policy_resolution(NotificationAttribution::unresolved(
        claim.reported_name,
        AttributionReason::MissingSenderEvidence,
        &detail,
        sender_claim_group_key(AttributionStatus::Unresolved, claim.reported_name, sender),
    ));
    with_diagnostics(
        resolution,
        claim,
        sender,
        None,
        LaunchVerification::InsufficientEvidence(LaunchFailure::MissingSenderEvidence),
    )
}

pub(in crate::daemon) async fn resolve_attribution(
    claim: AppClaim<'_>,
    sender: &SenderMetadata,
    index: &DesktopIdentityIndex,
    connection: &Connection,
) -> AttributionResolution {
    let mut owned_desktop_ids = HashSet::new();
    // Well-known ownership remains supporting context rather than application authority
    if let (Some(sender_name), Some(desktop_id)) = (
        sender.sender_name.as_deref(),
        claim.desktop_entry.and_then(validate_desktop_id),
    ) {
        let records = index.records_for_id(&desktop_id);
        if records.iter().any(|record| record.dbus_activatable)
            && sender_owns_name(connection, sender_name, &desktop_id).await
        {
            owned_desktop_ids.insert(normalize_desktop_id(&desktop_id));
        }
    }

    // Cached process data is refreshed before it affects attribution
    let mut sender = refresh_sender_security_evidence(sender);
    let initial = resolve_with_evidence(claim, &sender, index, &owned_desktop_ids);
    if sender.install_provenance.is_known()
        || !matches!(
            initial.attribution.status,
            unixnotis_core::AttributionStatus::Recognized
        )
    {
        return initial;
    }

    // Ownership is needed only to distinguish a probable helper from a different installed app
    enrich_sender_install_provenance(&mut sender, index).await;
    resolve_with_evidence(claim, &sender, index, &owned_desktop_ids)
}

async fn enrich_sender_install_provenance(
    sender: &mut SenderMetadata,
    index: &DesktopIdentityIndex,
) {
    if sender.install_provenance.is_known() {
        return;
    }
    let (Some(path), Some(sender_identity)) = (
        sender.sender_executable.as_deref(),
        sender.sender_executable_identity,
    ) else {
        return;
    };
    if !sender_identity.is_system_managed() || !sender_identity.is_executable_regular() {
        return;
    }
    let Some(current) = executable_evidence_for_path(std::path::Path::new(path)) else {
        return;
    };
    if !current_system_identity_matches_sender(current.identity, sender_identity) {
        return;
    }
    sender.install_provenance = index
        .install_provenance_for_path_async(current.canonical_path)
        .await;
}

fn resolve_with_evidence(
    claim: AppClaim<'_>,
    sender: &SenderMetadata,
    index: &DesktopIdentityIndex,
    _owned_desktop_ids: &HashSet<String>,
) -> AttributionResolution {
    let desktop_entry = claim.desktop_entry.and_then(validate_desktop_id);
    let hint_records = desktop_entry
        .as_deref()
        .map_or_else(Vec::new, |desktop_id| index.records_for_id(desktop_id));
    if desktop_entry.is_some()
        && !hint_records.is_empty()
        && claim.reported_name.trim().is_empty()
        && trusted_portal_path(sender, index).is_some()
    {
        // A trusted portal executable may forward its broker-owned application id
        let record = preferred_record(&hint_records);
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

async fn sender_owns_name(connection: &Connection, sender_name: &str, desktop_id: &str) -> bool {
    let Ok(bus_name) = zbus::names::BusName::try_from(desktop_id) else {
        return false;
    };
    let Ok(proxy) = DBusProxy::new(connection).await else {
        return false;
    };
    proxy
        .get_name_owner(bus_name)
        .await
        .is_ok_and(|owner| owner.as_str() == sender_name)
}

pub(super) fn validate_desktop_id(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty()
        || value.len() > MAX_DESKTOP_ID_BYTES
        || value.contains(['/', '\\', '\0'])
        || value.chars().any(char::is_control)
    {
        return None;
    }
    let value = value.strip_suffix(".desktop").unwrap_or(value);
    if value == "." || value == ".." || value.is_empty() {
        return None;
    }
    Some(value.to_string())
}

#[cfg(test)]
#[path = "tests/resolver.rs"]
mod tests;
