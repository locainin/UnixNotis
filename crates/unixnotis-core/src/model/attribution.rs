//! Authenticated notification identity projected from daemon-owned sender metadata

use std::path::Path;

use serde::{Deserialize, Serialize};
use zbus::zvariant::Type;

use crate::util;

const MAX_ATTRIBUTION_NAME_BYTES: usize = 256;

/// Identity details displayed separately from caller-controlled notification content
#[derive(Debug, Clone, Default, Serialize, Deserialize, Type, PartialEq, Eq)]
pub struct NotificationAttribution {
    // False means the claimed app name could not be tied to the sender executable
    pub verified: bool,
    // A mismatched caller claim remains visible as secondary metadata
    pub reported_name: String,
    // The authenticated executable basename drives application badge lookup
    pub badge_icon: String,
}

impl NotificationAttribution {
    /// Resolve the primary display name and attribution from authenticated process metadata
    #[must_use]
    pub fn resolve(reported_name: &str, sender_executable: Option<&str>) -> (String, Self) {
        let reported_name = bounded_name(reported_name);
        let Some(executable_name) = sender_executable.and_then(executable_name) else {
            // Unresolved senders keep their claim visible but never gain verified status
            let display_name = fallback_display_name(&reported_name);
            return (
                display_name,
                Self {
                    verified: false,
                    reported_name: String::new(),
                    badge_icon: "dialog-warning-symbolic".to_string(),
                },
            );
        };

        let executable_name = bounded_name(executable_name);
        let verified = identity_names_match(&reported_name, &executable_name);
        if verified {
            return (
                fallback_display_name(&reported_name),
                Self {
                    verified: true,
                    reported_name: String::new(),
                    badge_icon: executable_name,
                },
            );
        }

        // Mismatches lead with authenticated process identity and retain the claim second
        (
            fallback_display_name(&executable_name),
            Self {
                verified: false,
                reported_name,
                badge_icon: executable_name,
            },
        )
    }

    /// Build a visible one-line identity label for popup and notification-center headers
    #[must_use]
    pub fn display_label(&self, primary_name: &str) -> String {
        if self.verified {
            return primary_name.to_string();
        }
        if self.reported_name.is_empty() {
            return format!("{primary_name} · unverified");
        }
        format!("{primary_name} · unverified claim: {}", self.reported_name)
    }
}

fn executable_name(path: &str) -> Option<&str> {
    Path::new(path)
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
}

fn identity_names_match(reported_name: &str, executable_name: &str) -> bool {
    let reported = normalized_identity(reported_name);
    let executable = normalized_identity(executable_name);
    !reported.is_empty() && reported == executable
}

fn normalized_identity(value: &str) -> String {
    // Spaces and ordinary separators differ between desktop names and executable filenames
    value
        .chars()
        .filter(|ch| ch.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn bounded_name(value: &str) -> String {
    let clean = util::sanitize_inline_display_text(value);
    util::truncate_utf8_bytes(clean.trim(), MAX_ATTRIBUTION_NAME_BYTES)
}

fn fallback_display_name(value: &str) -> String {
    if value.is_empty() {
        "Unknown application".to_string()
    } else {
        value.to_string()
    }
}

#[cfg(test)]
#[path = "tests/attribution.rs"]
mod tests;
