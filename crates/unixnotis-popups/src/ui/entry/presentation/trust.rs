//! Human-scale trust state derived without exposing raw provenance in the card body

use unixnotis_core::{AttributionClass, InlineReplyPolicy, NotificationView};

/// Small set of trust states used by popup styling and interaction hints
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::ui::entry) enum TrustLevel {
    Verified,
    Unverified,
    Suspicious,
    System,
}

impl TrustLevel {
    pub(in crate::ui::entry) const fn css_class(self) -> &'static str {
        match self {
            Self::Verified => "verified",
            Self::Unverified => "unverified",
            Self::Suspicious => "suspicious",
            Self::System => "system",
        }
    }
}

/// Safe visible trust text plus optional diagnostic detail for a tooltip
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::ui::entry) struct PopupTrustPresentation {
    pub(in crate::ui::entry) level: TrustLevel,
    pub(in crate::ui::entry) short_label: Option<String>,
    pub(in crate::ui::entry) details_label: Option<String>,
    pub(in crate::ui::entry) allow_reply: bool,
    pub(in crate::ui::entry) show_reply_unavailable: bool,
}

impl PopupTrustPresentation {
    pub(in crate::ui::entry) fn for_notification(notification: &NotificationView) -> Self {
        let level = trust_level(notification);
        let short_label = short_trust_label(level).map(str::to_string);
        let details_label = nonempty_text(&notification.attribution.source_label);
        let has_reply = notification.inline_reply.available
            || notification
                .actions
                .iter()
                .any(|action| action.key == "inline-reply");
        let allow_reply = has_reply
            && notification.inline_reply_policy == InlineReplyPolicy::Allow
            && level == TrustLevel::Verified;

        Self {
            level,
            short_label,
            details_label,
            allow_reply,
            // Explain a missing reply control only when the sender actually requested one
            show_reply_unavailable: has_reply && !allow_reply,
        }
    }
}

const fn trust_level(notification: &NotificationView) -> TrustLevel {
    // Explicit conflicts outrank the weaker association class carried beside them
    if notification.attribution.has_warning() {
        return TrustLevel::Suspicious;
    }

    match notification.attribution.class {
        AttributionClass::SystemAssociated | AttributionClass::PortalAssociated => {
            TrustLevel::Verified
        }
        AttributionClass::UserAssociated | AttributionClass::Unknown => TrustLevel::Unverified,
        AttributionClass::TrustedRelay => TrustLevel::System,
        AttributionClass::Conflict => TrustLevel::Suspicious,
    }
}

const fn short_trust_label(level: TrustLevel) -> Option<&'static str> {
    match level {
        // Verified application identity stays quiet unless a future theme opts into a marker
        TrustLevel::Verified => None,
        TrustLevel::Unverified => Some("Unverified"),
        TrustLevel::Suspicious => Some("Suspicious"),
        TrustLevel::System => Some("Command-line tool"),
    }
}

fn nonempty_text(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}
