//! Ordered application association from desktop hints, bus ownership, and file identity

use std::collections::HashSet;

use unixnotis_core::{AttributionClass, InlineReplyPolicy, NotificationAttribution};
use zbus::fdo::DBusProxy;
use zbus::Connection;

use super::desktop_index::{
    normalize_desktop_id, normalize_name, verify_record_launch, DesktopIdentityIndex,
    DesktopRecord, LaunchFailure, LaunchVerification,
};
use super::executable::{executable_evidence_for_path, FileIdentity};
use super::policy::inline_reply_policy;
use super::sender::{refresh_sender_security_evidence, SenderMetadata};

const MAX_DESKTOP_ID_BYTES: usize = 256;

#[derive(Clone, Copy)]
pub(in crate::daemon) struct AppClaim<'a> {
    pub(in crate::daemon) reported_name: &'a str,
    pub(in crate::daemon) desktop_entry: Option<&'a str>,
}

pub(in crate::daemon) struct AttributionResolution {
    pub(in crate::daemon) attribution: NotificationAttribution,
    pub(in crate::daemon) inline_reply_policy: InlineReplyPolicy,
}

#[derive(Clone, Copy)]
struct VerifiedDesktopRecord<'record>(&'record DesktopRecord);

#[derive(Clone, Copy)]
struct CandidateVerification<'record> {
    record: &'record DesktopRecord,
    verification: LaunchVerification,
}

pub(in crate::daemon) fn unknown_reply_denied(
    claim: AppClaim<'_>,
    sender: &SenderMetadata,
    reason: &str,
) -> AttributionResolution {
    let source = sender.sender_executable.as_deref().map_or_else(
        || reason.to_string(),
        |path| format!("{reason}; source {path}"),
    );
    AttributionResolution {
        attribution: NotificationAttribution::unknown(
            claim.reported_name,
            &source,
            unknown_group_key(claim.reported_name, sender),
        ),
        inline_reply_policy: InlineReplyPolicy::Deny,
    }
}

pub(in crate::daemon) async fn resolve_attribution(
    claim: AppClaim<'_>,
    sender: &SenderMetadata,
    index: &DesktopIdentityIndex,
    connection: &Connection,
) -> AttributionResolution {
    let mut owned_desktop_ids = HashSet::new();
    // Bus ownership is collected as diagnostic context and never replaces file evidence
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
    // Cached D-Bus metadata is refreshed before it can grant application authority
    let sender = refresh_sender_security_evidence(sender);
    resolve_with_evidence(claim, &sender, index, &owned_desktop_ids)
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
        // Portal backends forward a broker-verified app id as desktop-entry
        return resolution_for_portal_record(hint_records[0], sender, index);
    }

    // Hint and live-executable candidates are evaluated together so weak metadata cannot win early
    let mut candidates = hint_records.clone();
    if let Some(identity) = sender.sender_executable_identity {
        candidates.extend(index.records_for_executable(identity));
    }
    candidates.dedup_by(|left, right| std::ptr::eq(*left, *right));
    let results = candidates
        .iter()
        .map(|record| CandidateVerification {
            record,
            verification: verify_record_sender(record, sender, index),
        })
        .collect::<Vec<_>>();
    if let Some(record) = strongest_verified_result(&results, claim.reported_name) {
        return resolution_for_record(record, claim.reported_name, sender, index);
    }

    if let Some(identity) = sender.sender_executable_identity {
        if let Some(path) = index.trusted_relay_path(identity) {
            // Relay groups include both relay identity and the relayed claim
            let group_key = format!(
                "relay:{}:{}",
                identity.group_fragment(),
                normalize_name(claim.reported_name)
            );
            let attribution = NotificationAttribution::trusted_relay(
                claim.reported_name,
                &format!("Sent via {}", path.display()),
                index.claim_matches_system_app(claim.reported_name),
                group_key,
            );
            return policy_resolution(attribution);
        }
    }

    let hint_is_definitive = !hint_records.is_empty()
        && results
            .iter()
            .filter(|result| {
                hint_records
                    .iter()
                    .any(|record| std::ptr::eq(*record, result.record))
            })
            .all(CandidateVerification::is_definitive_mismatch);
    let matching_system_is_definitive = results.iter().any(|result| {
        result.record.system_association
            && result.record.claim_matches(claim.reported_name)
            && result.is_definitive_mismatch()
    });
    if hint_is_definitive || matching_system_is_definitive {
        return conflict_resolution(
            claim.reported_name,
            sender,
            launch_failure_label(
                results
                    .iter()
                    .find(|result| result.is_definitive_mismatch())
                    .map_or(
                        LaunchFailure::DesktopClaimMismatch,
                        CandidateVerification::failure,
                    ),
            ),
        );
    }

    let matching_claim_has_insufficient_evidence = results.iter().any(|result| {
        result.record.claim_matches(claim.reported_name)
            && matches!(
                result.verification,
                LaunchVerification::InsufficientEvidence(_)
            )
    });
    if index.claim_matches_system_app(claim.reported_name)
        && !matching_claim_has_insufficient_evidence
    {
        // Protected branding without the matching executable is an explicit conflict
        return conflict_resolution(claim.reported_name, sender, "executable identity mismatch");
    }

    let source = sender
        .sender_executable
        .as_deref()
        .map(|path| format!("Source: {path}"))
        .unwrap_or_default();
    let group_key = unknown_group_key(claim.reported_name, sender);
    policy_resolution(NotificationAttribution::unknown(
        claim.reported_name,
        &source,
        group_key,
    ))
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

fn strongest_verified_result<'record>(
    results: &[CandidateVerification<'record>],
    reported_name: &str,
) -> Option<VerifiedDesktopRecord<'record>> {
    let missing_name = reported_name.trim().is_empty();
    let mut verified = results.iter().filter(|result| {
        matches!(result.verification, LaunchVerification::Verified(_))
            && (missing_name || result.record.claim_matches(reported_name))
    });
    let first = verified.next()?;
    let mut preferred = first;
    for candidate in verified {
        let preferred_rank = record_trust_rank(preferred.record);
        let candidate_rank = record_trust_rank(candidate.record);
        if candidate_rank > preferred_rank {
            preferred = candidate;
            continue;
        }
        if candidate_rank == preferred_rank
            && normalize_desktop_id(&candidate.record.id)
                != normalize_desktop_id(&preferred.record.id)
        {
            // Equal-strength records for distinct applications remain ambiguous
            return None;
        }
    }
    Some(VerifiedDesktopRecord(preferred.record))
}

const fn record_trust_rank(record: &DesktopRecord) -> u8 {
    if record.system_association {
        2
    } else {
        1
    }
}

const fn launch_failure_label(reason: LaunchFailure) -> &'static str {
    match reason {
        LaunchFailure::MissingCommandLine => "missing command-line evidence",
        LaunchFailure::UnstructuredCommandLine => "unstructured command-line evidence",
        LaunchFailure::UnsupportedWrapper => "unsupported launch wrapper",
        LaunchFailure::AmbiguousDesktopAssociation => "ambiguous desktop association",
        LaunchFailure::DynamicOnlyContract => "dynamic-only launch contract",
        LaunchFailure::ExecutableMismatch => "executable identity mismatch",
        LaunchFailure::ProtectedPayloadMismatch => "protected application payload mismatch",
        LaunchFailure::RequiredArgumentMismatch => "required launch argument mismatch",
        LaunchFailure::DesktopClaimMismatch => "desktop claim mismatch",
    }
}

fn resolution_for_portal_record(
    record: &DesktopRecord,
    sender: &SenderMetadata,
    index: &DesktopIdentityIndex,
) -> AttributionResolution {
    let portal = sender
        .sender_executable_identity
        .and_then(|_| trusted_portal_path(sender, index))
        .map_or_else(
            || "desktop portal".to_string(),
            |path| path.display().to_string(),
        );
    let group_key = format!("portal-desktop:{}", record.id);
    let attribution = NotificationAttribution::associated(
        &record.display_name,
        &record.id,
        &record.badge_icon,
        &format!("Mediated by {portal}"),
        AttributionClass::PortalAssociated,
        false,
        group_key,
    );
    policy_resolution(attribution)
}

fn trusted_portal_path<'index>(
    sender: &SenderMetadata,
    index: &'index DesktopIdentityIndex,
) -> Option<&'index std::path::Path> {
    let identity = sender.sender_executable_identity?;
    let path = std::path::Path::new(sender.sender_executable.as_deref()?);
    index.trusted_portal_path(identity, path)
}

fn resolution_for_record(
    verified: VerifiedDesktopRecord<'_>,
    reported_name: &str,
    sender: &SenderMetadata,
    index: &DesktopIdentityIndex,
) -> AttributionResolution {
    let record = verified.0;
    // Display metadata is projected only after the record and sender identities agree
    if !reported_name.trim().is_empty() && !record.claim_matches(reported_name) {
        return conflict_resolution(reported_name, sender, "application claim mismatch");
    }
    let class = if record.system_association {
        AttributionClass::SystemAssociated
    } else {
        AttributionClass::UserAssociated
    };
    let source_label = record
        .executable_path
        .as_deref()
        .map(|path| path.display().to_string())
        .unwrap_or_default();
    let shadows_system_id = !record.system_origin && index.has_system_record_for_id(&record.id);
    let source_label = if shadows_system_id {
        format!("Shadows a system desktop entry; source {source_label}")
    } else {
        source_label
    };
    let group_prefix = if record.system_association {
        "system-desktop"
    } else if record.system_origin {
        "system-unverified-desktop"
    } else {
        "user-desktop"
    };
    let origin = record.desktop_identity.map_or_else(
        || "unknown".to_string(),
        super::executable::FileIdentity::group_fragment,
    );
    let group_key = if record.system_association {
        format!("{group_prefix}:{}", record.id)
    } else {
        format!("{group_prefix}:{origin}:{}", record.id)
    };
    let attribution = NotificationAttribution::associated(
        &record.display_name,
        &record.id,
        &record.badge_icon,
        &source_label,
        class,
        shadows_system_id,
        group_key,
    );
    policy_resolution(attribution)
}

fn conflict_resolution(
    reported_name: &str,
    sender: &SenderMetadata,
    reason: &str,
) -> AttributionResolution {
    let source = sender.sender_executable.as_deref().map_or_else(
        || reason.to_string(),
        |path| format!("{reason}; source {path}"),
    );
    policy_resolution(NotificationAttribution::conflict(
        reported_name,
        &source,
        unknown_group_key(reported_name, sender),
    ))
}

const fn policy_resolution(attribution: NotificationAttribution) -> AttributionResolution {
    // Interaction policy remains separate so presentation changes cannot enable replies
    AttributionResolution {
        inline_reply_policy: inline_reply_policy(attribution.class),
        attribution,
    }
}

fn verify_record_sender(
    record: &DesktopRecord,
    sender: &SenderMetadata,
    index: &DesktopIdentityIndex,
) -> LaunchVerification {
    if !record.association_eligible {
        return LaunchVerification::InsufficientEvidence(LaunchFailure::UnsupportedWrapper);
    }
    let (Some(record_identity), Some(sender_identity)) = (
        record.executable_identity,
        sender.sender_executable_identity,
    ) else {
        return LaunchVerification::InsufficientEvidence(LaunchFailure::ExecutableMismatch);
    };
    if !record_identity.same_file(sender_identity) {
        return LaunchVerification::DefinitiveMismatch(LaunchFailure::ExecutableMismatch);
    }

    if record.system_association {
        // Cached inode equality cannot carry root ownership across inode reuse
        if !sender_identity.is_system_managed() || !sender_identity.is_executable_regular() {
            return LaunchVerification::InsufficientEvidence(LaunchFailure::ExecutableMismatch);
        }
        let Some(path) = record.executable_path.as_deref() else {
            return LaunchVerification::InsufficientEvidence(LaunchFailure::ExecutableMismatch);
        };
        // Reopen the installed path so stale index authority cannot outlive replacement
        let Some(current) = executable_evidence_for_path(path) else {
            return LaunchVerification::InsufficientEvidence(LaunchFailure::ExecutableMismatch);
        };
        if !current_system_identity_matches_sender(current.identity, sender_identity) {
            return LaunchVerification::InsufficientEvidence(LaunchFailure::ExecutableMismatch);
        }
    }

    verify_record_launch(record, index, sender_identity, &sender.command_line)
}

const fn current_system_identity_matches_sender(
    current: FileIdentity,
    sender_identity: FileIdentity,
) -> bool {
    // Every property is checked again because the cached inode may have changed in place
    current.same_file(sender_identity)
        && current.is_system_managed()
        && current.is_executable_regular()
}

fn unknown_group_key(reported_name: &str, sender: &SenderMetadata) -> String {
    // Unknown senders cannot merge into a trusted desktop group by copying its name
    let claim = normalize_name(reported_name);
    sender.sender_executable_identity.map_or_else(
        || format!("unknown:{claim}"),
        |identity| format!("executable:{}:{claim}", identity.group_fragment()),
    )
}

async fn sender_owns_name(connection: &Connection, sender_name: &str, desktop_id: &str) -> bool {
    // Invalid well-known names are rejected before contacting the bus daemon
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
    // Desktop hints stay short, single-component, and safe for later lookups
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
