//! Structured notification attribution and interaction policy

use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use zbus::zvariant::Type;

use crate::util;

const MAX_ATTRIBUTION_TEXT_BYTES: usize = 256;
const MAX_GROUP_KEY_BYTES: usize = 512;

/// Daemon-owned result of application identity evaluation
// Representation-aware Serde keeps each wire value at one byte
#[derive(Debug, Copy, Clone, Default, Serialize_repr, Deserialize_repr, Type, PartialEq, Eq)]
#[repr(u8)]
pub enum AttributionStatus {
    Verified = 0,
    Recognized = 1,
    #[default]
    Unresolved = 2,
    Conflict = 3,
    Relay = 4,
}

/// Stable reason for one attribution result
// Numeric ranges keep positive, uncertain, and contradictory evidence easy to inspect
#[derive(Debug, Copy, Clone, Default, Serialize_repr, Deserialize_repr, Type, PartialEq, Eq)]
#[repr(u8)]
pub enum AttributionReason {
    ExactSystemExecutable = 0,
    VerifiedPortalAppId = 1,
    ExactUserExecutable = 2,
    VerifiedProtectedPayload = 3,
    TrustedRelayExecutable = 4,

    #[default]
    MissingSenderEvidence = 10,
    MissingCommandLine = 11,
    AmbiguousDesktopRecords = 12,
    DynamicLaunchContract = 13,
    UnsupportedWrapper = 14,
    NoDesktopCandidate = 15,

    ExecutableMismatch = 20,
    ProtectedPayloadMismatch = 21,
    ApplicationClaimMismatch = 22,
}

/// Independent policy for credential-like inline text controls
// Value one stays unused until confirmation is enforced by the daemon
#[derive(Debug, Copy, Clone, Default, Serialize_repr, Deserialize_repr, Type, PartialEq, Eq)]
#[repr(u8)]
pub enum InlineReplyPolicy {
    Allow = 0,
    #[default]
    Deny = 2,
}

/// Backend policy for application-owned action signals
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ApplicationActionPolicy {
    Allow,
    Confirm,
    Deny,
}

/// Application identity selected from sender and desktop evidence
#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq)]
pub struct NotificationAttribution {
    // The primary label is always selected by the daemon
    pub display_name: String,
    // The protocol app_name stays visible without becoming identity evidence
    pub claimed_name: String,
    // Empty strings represent optional wire fields that were not resolved
    pub desktop_id: String,
    pub badge_icon: String,
    // Status and reason carry state without parsing diagnostic text
    pub status: AttributionStatus,
    pub reason: AttributionReason,
    // Human-readable detail is display-only and never interpreted by clients
    pub diagnostic_detail: String,
    // The daemon owns grouping so copied labels cannot join trusted groups
    pub group_key: String,
}

impl Default for NotificationAttribution {
    fn default() -> Self {
        Self {
            display_name: "Unknown application".to_string(),
            claimed_name: String::new(),
            desktop_id: String::new(),
            badge_icon: "application-x-executable-symbolic".to_string(),
            status: AttributionStatus::Unresolved,
            reason: AttributionReason::MissingSenderEvidence,
            diagnostic_detail: String::new(),
            group_key: "unknown".to_string(),
        }
    }
}

impl NotificationAttribution {
    /// Build a strongly bound application identity
    #[must_use]
    pub fn verified(
        display_name: &str,
        claimed_name: &str,
        desktop_id: &str,
        badge_icon: &str,
        reason: AttributionReason,
        diagnostic_detail: &str,
        group_key: String,
    ) -> Self {
        Self::resolved(
            display_name,
            claimed_name,
            desktop_id,
            badge_icon,
            AttributionStatus::Verified,
            reason,
            diagnostic_detail,
            group_key,
        )
    }

    /// Build a known but non-authoritative application identity
    #[must_use]
    pub fn recognized(
        display_name: &str,
        claimed_name: &str,
        desktop_id: &str,
        badge_icon: &str,
        reason: AttributionReason,
        diagnostic_detail: &str,
        group_key: String,
    ) -> Self {
        Self::resolved(
            display_name,
            claimed_name,
            desktop_id,
            badge_icon,
            AttributionStatus::Recognized,
            reason,
            diagnostic_detail,
            group_key,
        )
    }

    /// Build an attribution without a reliable desktop association
    #[must_use]
    pub fn unresolved(
        claimed_name: &str,
        reason: AttributionReason,
        diagnostic_detail: &str,
        group_key: String,
    ) -> Self {
        Self::resolved(
            "Unknown application",
            claimed_name,
            "",
            "application-x-executable-symbolic",
            AttributionStatus::Unresolved,
            reason,
            diagnostic_detail,
            group_key,
        )
    }

    /// Build an attribution backed by a concrete contradictory candidate
    #[must_use]
    pub fn conflict(
        claimed_name: &str,
        desktop_id: &str,
        reason: AttributionReason,
        diagnostic_detail: &str,
        group_key: String,
    ) -> Self {
        debug_assert!(
            matches!(
                reason,
                AttributionReason::ExecutableMismatch
                    | AttributionReason::ProtectedPayloadMismatch
                    | AttributionReason::ApplicationClaimMismatch
            ),
            "conflict attribution requires a concrete contradiction reason"
        );
        Self::resolved(
            "Unknown application",
            claimed_name,
            desktop_id,
            "dialog-warning-symbolic",
            AttributionStatus::Conflict,
            reason,
            diagnostic_detail,
            group_key,
        )
    }

    /// Build a known relay identity without authenticating its app label
    #[must_use]
    pub fn relay(claimed_name: &str, diagnostic_detail: &str, group_key: String) -> Self {
        Self::resolved(
            "Command-line notification",
            claimed_name,
            "",
            "utilities-terminal-symbolic",
            AttributionStatus::Relay,
            AttributionReason::TrustedRelayExecutable,
            diagnostic_detail,
            group_key,
        )
    }

    #[expect(
        clippy::needless_pass_by_value,
        clippy::too_many_arguments,
        reason = "the wire fields stay explicit at construction"
    )]
    fn resolved(
        display_name: &str,
        claimed_name: &str,
        desktop_id: &str,
        badge_icon: &str,
        status: AttributionStatus,
        reason: AttributionReason,
        diagnostic_detail: &str,
        group_key: String,
    ) -> Self {
        Self {
            display_name: display_name_or_unknown(display_name),
            claimed_name: bounded_text(claimed_name),
            desktop_id: bounded_text(desktop_id),
            badge_icon: bounded_text(badge_icon),
            status,
            reason,
            diagnostic_detail: bounded_text(diagnostic_detail),
            group_key: bounded_group_key(&group_key),
        }
    }

    /// Policy for signals that belong to the authenticated application
    #[must_use]
    pub const fn application_action_policy(&self) -> ApplicationActionPolicy {
        if matches!(self.status, AttributionStatus::Verified) {
            ApplicationActionPolicy::Allow
        } else {
            ApplicationActionPolicy::Deny
        }
    }
}

fn bounded_text(value: &str) -> String {
    let clean = util::sanitize_inline_display_text(value);
    util::truncate_utf8_bytes(clean.trim(), MAX_ATTRIBUTION_TEXT_BYTES)
}

fn bounded_group_key(value: &str) -> String {
    let clean = util::sanitize_inline_display_text(value);
    let bounded = util::truncate_utf8_bytes(clean.trim(), MAX_GROUP_KEY_BYTES);
    if bounded.is_empty() {
        "unknown".to_string()
    } else {
        bounded
    }
}

fn display_name_or_unknown(value: &str) -> String {
    let value = bounded_text(value);
    if value.is_empty() {
        "Unknown application".to_string()
    } else {
        value
    }
}

#[cfg(test)]
#[path = "tests/attribution.rs"]
mod tests;
