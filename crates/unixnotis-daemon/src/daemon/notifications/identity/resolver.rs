//! Ordered application association from desktop hints, bus ownership, and file identity

use std::collections::HashSet;

use unixnotis_core::{AttributionClass, InlineReplyPolicy, NotificationAttribution};
use zbus::fdo::DBusProxy;
use zbus::Connection;

use super::desktop_index::{
    normalize_desktop_id, normalize_name, record_launch_matches, DesktopIdentityIndex,
    DesktopRecord,
};
use super::executable::{executable_evidence_for_path, FileIdentity};
use super::policy::inline_reply_policy;
use super::sender::SenderMetadata;

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
    resolve_with_evidence(claim, sender, index, &owned_desktop_ids)
}

fn resolve_with_evidence(
    claim: AppClaim<'_>,
    sender: &SenderMetadata,
    index: &DesktopIdentityIndex,
    owned_desktop_ids: &HashSet<String>,
) -> AttributionResolution {
    // An explicit desktop hint is accepted only when its executable is the sender file
    let desktop_entry = claim.desktop_entry.and_then(validate_desktop_id);
    let mut desktop_hint_conflict = None;
    if let Some(desktop_id) = desktop_entry.as_deref() {
        let records = index.records_for_id(desktop_id);
        if !records.is_empty() {
            if claim.reported_name.trim().is_empty()
                && sender
                    .sender_executable_identity
                    .and_then(|identity| index.trusted_portal_path(identity))
                    .is_some()
            {
                // Portal backends forward a broker-verified app id as desktop-entry
                return resolution_for_portal_record(records[0], sender, index);
            }
            if let Some(record) = records
                .iter()
                .find_map(|record| verify_record_sender(record, sender, index))
            {
                return resolution_for_record(record, claim.reported_name, sender, index);
            }
            if records
                .iter()
                .any(|record| owned_desktop_ids.contains(&normalize_desktop_id(&record.id)))
            {
                // Session applications can request names, so ownership is context rather than proof
                desktop_hint_conflict = Some("bus name ownership lacks executable association");
            } else {
                // Packaging aliases may be stale, so exact executable evidence still gets a chance
                desktop_hint_conflict = Some("desktop identity mismatch");
            }
        }
    }

    if let Some(identity) = sender.sender_executable_identity {
        // Exact file association is stronger than every caller-controlled application name
        let records = index.records_for_executable(identity);
        if let Some(record) =
            verified_executable_record(&records, claim.reported_name, sender, index)
        {
            return resolution_for_record(record, claim.reported_name, sender, index);
        }
        if records
            .iter()
            .any(|record| record.system_association && record_matches_sender(record, sender, index))
        {
            // A known executable with a conflicting name must fail closed
            return conflict_resolution(claim.reported_name, sender, "application claim mismatch");
        }
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

    if let Some(reason) = desktop_hint_conflict {
        return conflict_resolution(claim.reported_name, sender, reason);
    }

    if index.claim_matches_system_app(claim.reported_name) {
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

fn resolution_for_portal_record(
    record: &DesktopRecord,
    sender: &SenderMetadata,
    index: &DesktopIdentityIndex,
) -> AttributionResolution {
    let portal = sender
        .sender_executable_identity
        .and_then(|identity| index.trusted_portal_path(identity))
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

fn verified_executable_record<'record>(
    records: &[&'record DesktopRecord],
    reported_name: &str,
    sender: &SenderMetadata,
    index: &DesktopIdentityIndex,
) -> Option<VerifiedDesktopRecord<'record>> {
    let missing_name = reported_name.trim().is_empty();
    let mut matches = records.iter().filter_map(|record| {
        (missing_name || record.claim_matches(reported_name))
            .then(|| verify_record_sender(record, sender, index))
            .flatten()
    });
    let first = matches.next()?;
    let first_id = normalize_desktop_id(&first.0.id);
    let mut preferred = first;

    for candidate in matches {
        // One executable cannot prove which of two distinct desktop applications sent the message
        if normalize_desktop_id(&candidate.0.id) != first_id {
            return None;
        }
        // Protected records win over duplicate user metadata for the same desktop id
        if candidate.0.system_association && !preferred.0.system_association {
            preferred = candidate;
        }
    }
    Some(preferred)
}

fn record_matches_sender(
    record: &DesktopRecord,
    sender: &SenderMetadata,
    index: &DesktopIdentityIndex,
) -> bool {
    if !record.association_eligible {
        return false;
    }
    let (Some(record_identity), Some(sender_identity)) = (
        record.executable_identity,
        sender.sender_executable_identity,
    ) else {
        return false;
    };
    if !record_identity.same_file(sender_identity) {
        return false;
    }

    if record.system_association {
        // Cached inode equality cannot carry root ownership across inode reuse
        if !sender_identity.is_system_managed() || !sender_identity.is_executable_regular() {
            return false;
        }
        let Some(path) = record.executable_path.as_deref() else {
            return false;
        };
        // Reopen the installed path so stale index authority cannot outlive replacement
        let Some(current) = executable_evidence_for_path(path) else {
            return false;
        };
        if !current_system_identity_matches_sender(current.identity, sender_identity) {
            return false;
        }
    }

    // Dedicated application binaries may add safe runtime flags after desktop activation
    !index.requires_launch_arguments(record)
        || record_launch_matches(record, sender_identity, sender.sender_cmdline.as_deref())
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

fn verify_record_sender<'record>(
    record: &'record DesktopRecord,
    sender: &SenderMetadata,
    index: &DesktopIdentityIndex,
) -> Option<VerifiedDesktopRecord<'record>> {
    // This wrapper makes sender launch verification mandatory at every association call site
    record_matches_sender(record, sender, index).then_some(VerifiedDesktopRecord(record))
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
