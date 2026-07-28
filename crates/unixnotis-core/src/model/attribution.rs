//! Notification application association and interaction policy

use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use zbus::zvariant::Type;

use crate::util;

const MAX_ATTRIBUTION_TEXT_BYTES: usize = 256;

/// Evidence class used to present an application without claiming universal authentication
// Representation-aware Serde keeps the D-Bus body aligned with its one-byte signature
#[derive(Debug, Copy, Clone, Default, Serialize_repr, Deserialize_repr, Type, PartialEq, Eq)]
#[repr(u8)]
pub enum AttributionClass {
    SystemAssociated = 0,
    PortalAssociated = 1,
    UserAssociated = 2,
    TrustedRelay = 3,
    #[default]
    Unknown = 4,
    Conflict = 5,
}

/// Independent policy for credential-like inline text controls
// Ordinary enum Serde writes a wider variant index that strict brokers reject
#[derive(Debug, Copy, Clone, Default, Serialize_repr, Deserialize_repr, Type, PartialEq, Eq)]
#[repr(u8)]
pub enum InlineReplyPolicy {
    Allow = 0,
    // Value 1 stays unused until confirmation is enforced by the daemon
    #[default]
    Deny = 2,
}

/// Application presentation derived by the daemon from sender and desktop metadata
#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq)]
pub struct NotificationAttribution {
    // Primary titles stay short and never contain diagnostics
    pub display_name: String,
    // Empty values represent unavailable optional D-Bus fields
    pub desktop_id: String,
    pub badge_icon: String,
    // Secondary source or warning text belongs in a tooltip or separate status element
    pub source_label: String,
    pub class: AttributionClass,
    // Risk presentation stays separate from association and interaction policy
    pub warning: bool,
    // Opaque daemon-built identity key prevents claimed names from merging trusted groups
    pub group_key: String,
}

impl Default for NotificationAttribution {
    fn default() -> Self {
        Self {
            display_name: "Unknown application".to_string(),
            desktop_id: String::new(),
            badge_icon: "dialog-warning-symbolic".to_string(),
            source_label: String::new(),
            class: AttributionClass::Unknown,
            warning: false,
            group_key: "unknown".to_string(),
        }
    }
}

impl NotificationAttribution {
    #[must_use]
    pub fn associated(
        display_name: &str,
        desktop_id: &str,
        badge_icon: &str,
        source_label: &str,
        class: AttributionClass,
        warning: bool,
        group_key: String,
    ) -> Self {
        Self {
            display_name: display_name_or_unknown(display_name),
            desktop_id: bounded_text(desktop_id),
            badge_icon: bounded_text(badge_icon),
            source_label: bounded_text(source_label),
            class,
            warning,
            group_key,
        }
    }

    #[must_use]
    pub fn unknown(display_name: &str, source_label: &str, group_key: String) -> Self {
        Self::associated(
            display_name,
            "",
            "dialog-question-symbolic",
            source_label,
            AttributionClass::Unknown,
            false,
            group_key,
        )
    }

    #[must_use]
    pub fn conflict(claimed_name: &str, source_label: &str, group_key: String) -> Self {
        let claim = display_name_or_unknown(claimed_name);
        Self::associated(
            "Unknown application",
            "",
            "dialog-warning-symbolic",
            &format!("Claims to be {claim}; {source_label}"),
            AttributionClass::Conflict,
            true,
            group_key,
        )
    }

    #[must_use]
    pub fn trusted_relay(
        display_name: &str,
        source_label: &str,
        warning: bool,
        group_key: String,
    ) -> Self {
        Self::associated(
            display_name,
            "",
            "dialog-information-symbolic",
            source_label,
            AttributionClass::TrustedRelay,
            warning,
            group_key,
        )
    }

    #[must_use]
    pub const fn has_warning(&self) -> bool {
        self.warning
    }

    /// Whether application-owned actions may be sent back to this notification source
    #[must_use]
    pub const fn allows_application_actions(&self) -> bool {
        // A warning means current evidence conflicts even when a weak association was found
        if self.warning {
            return false;
        }

        // Relay and unknown senders may display content but cannot receive trusted UI actions
        matches!(
            self.class,
            AttributionClass::SystemAssociated
                | AttributionClass::PortalAssociated
                | AttributionClass::UserAssociated
        )
    }
}

fn bounded_text(value: &str) -> String {
    let clean = util::sanitize_inline_display_text(value);
    util::truncate_utf8_bytes(clean.trim(), MAX_ATTRIBUTION_TEXT_BYTES)
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
