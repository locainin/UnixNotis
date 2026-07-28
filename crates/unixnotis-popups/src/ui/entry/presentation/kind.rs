//! Stable popup layout selection from protocol categories and trust state

use unixnotis_core::NotificationView;

use super::TrustLevel;

/// Visual structure used for one popup
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::ui::entry) enum PopupKind {
    Communication,
    Utility,
    Warning,
}

impl PopupKind {
    pub(in crate::ui::entry) fn for_notification(
        notification: &NotificationView,
        trust_level: TrustLevel,
    ) -> Self {
        // Conflicting identity needs its own restrained warning layout
        if trust_level == TrustLevel::Suspicious {
            return Self::Warning;
        }

        // Standard categories use class.specific, so only the first segment selects the layout
        let category_class = notification
            .category
            .split('.')
            .next()
            .unwrap_or_default()
            .trim();
        if communication_category_class(category_class)
            || notification.inline_reply.available
            || notification
                .actions
                .iter()
                .any(|action| action.key == "inline-reply")
        {
            return Self::Communication;
        }

        // Missing and vendor-specific categories stay compact instead of guessing from prose
        Self::Utility
    }

    pub(in crate::ui::entry) const fn css_class(self) -> &'static str {
        match self {
            Self::Communication => "communication",
            Self::Utility => "utility",
            Self::Warning => "warning",
        }
    }

    pub(in crate::ui::entry) const fn action_limit(self) -> usize {
        match self {
            Self::Communication => 3,
            Self::Utility | Self::Warning => 1,
        }
    }
}

fn communication_category_class(category_class: &str) -> bool {
    // Freedesktop communication classes are extended with common vendor spellings
    [
        "call",
        "email",
        "im",
        "presence",
        "chat",
        "message",
        "social",
        "voicemail",
    ]
    .iter()
    .any(|candidate| category_class.eq_ignore_ascii_case(candidate))
}
