//! Ordered application association from desktop hints, bus ownership, and file identity

use std::collections::HashSet;

use unixnotis_core::{AttributionClass, InlineReplyPolicy, NotificationAttribution};
use zbus::fdo::DBusProxy;
use zbus::Connection;

use super::super::sender::SenderMetadata;
use super::desktop_index::{
    normalize_desktop_id, normalize_name, DesktopIdentityIndex, DesktopRecord,
};
use super::policy::inline_reply_policy;

const MAX_DESKTOP_ID_BYTES: usize = 256;

pub(in crate::daemon) struct AppClaim<'a> {
    pub(in crate::daemon) reported_name: &'a str,
    pub(in crate::daemon) desktop_entry: Option<&'a str>,
}

pub(in crate::daemon) struct AttributionResolution {
    pub(in crate::daemon) attribution: NotificationAttribution,
    pub(in crate::daemon) inline_reply_policy: InlineReplyPolicy,
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
    if let Some(desktop_id) = desktop_entry.as_deref() {
        let records = index.records_for_id(desktop_id);
        if !records.is_empty() {
            if let Some(record) = records
                .iter()
                .find(|record| record_matches_sender(record, sender))
            {
                return resolution_for_record(record, claim.reported_name, sender);
            }
            if records
                .iter()
                .any(|record| owned_desktop_ids.contains(&normalize_desktop_id(&record.id)))
            {
                // Session applications can request names, so ownership is context rather than proof
                return conflict_resolution(
                    claim.reported_name,
                    sender,
                    "bus name ownership lacks executable association",
                );
            }
            return conflict_resolution(claim.reported_name, sender, "desktop identity mismatch");
        }
    }

    if let Some(identity) = sender.sender_executable_identity {
        // Exact file association is stronger than every caller-controlled application name
        let records = index.records_for_executable(identity);
        if let Some(record) = records
            .iter()
            .find(|record| record.claim_matches(claim.reported_name))
        {
            return resolution_for_record(record, claim.reported_name, sender);
        }
        if records.iter().any(|record| record.system_entry) {
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

fn resolution_for_record(
    record: &DesktopRecord,
    reported_name: &str,
    sender: &SenderMetadata,
) -> AttributionResolution {
    // Display metadata is projected only after the record and sender identities agree
    if !record.claim_matches(reported_name) {
        return conflict_resolution(reported_name, sender, "application claim mismatch");
    }
    let class = if record.system_entry {
        AttributionClass::SystemAssociated
    } else {
        AttributionClass::UserAssociated
    };
    let source_label = record
        .executable_path
        .as_deref()
        .map(|path| path.display().to_string())
        .unwrap_or_default();
    let attribution = NotificationAttribution::associated(
        &record.display_name,
        &record.id,
        &record.badge_icon,
        &source_label,
        class,
        false,
        format!("desktop:{}", record.id),
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

const fn record_matches_sender(record: &DesktopRecord, sender: &SenderMetadata) -> bool {
    match (
        record.executable_identity,
        sender.sender_executable_identity,
    ) {
        (Some(record_identity), Some(sender_identity)) => {
            record_identity.same_file(sender_identity)
        }
        _ => false,
    }
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
