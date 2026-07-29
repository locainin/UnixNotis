//! Plain presentation types shared without GTK widget ownership

/// Stable content hierarchy selected from protocol and trust evidence
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationKind {
    Communication,
    Utility,
    Media,
}

impl NotificationKind {
    #[must_use]
    pub fn for_notification(notification: &unixnotis_core::NotificationView) -> Self {
        super::build::notification_kind(notification)
    }

    #[must_use]
    pub const fn action_limit(self) -> usize {
        // Two visible actions preserve room for content; remaining actions use overflow
        match self {
            Self::Communication | Self::Utility | Self::Media => 2,
        }
    }

    #[must_use]
    pub const fn css_class(self) -> &'static str {
        match self {
            Self::Communication => "communication",
            Self::Utility => "utility",
            Self::Media => "media",
        }
    }
}

/// Human-scale trust state shown consistently by every notification client
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrustLevel {
    Verified,
    Recognized,
    Unresolved,
    Conflict,
    Relay,
}

impl TrustLevel {
    #[must_use]
    pub const fn css_class(self) -> &'static str {
        match self {
            Self::Verified => "verified",
            Self::Recognized => "recognized",
            Self::Unresolved => "unresolved",
            Self::Conflict => "conflict",
            Self::Relay => "relay",
        }
    }
}

/// Controlled badge source selected from daemon-owned identity evidence
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BadgePresentation {
    AuthenticatedApplication,
    RecognizedApplication,
    UnknownApplication,
    SuspiciousApplication,
    CommandLine,
    System,
}

/// Inline reply state kept separate from application-owned actions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplyPresentation {
    Hidden,
    Available,
    Unavailable,
}

/// Safe visible trust text plus optional diagnostic detail
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustPresentation {
    pub level: TrustLevel,
    pub short_label: Option<String>,
    pub details_label: Option<String>,
    pub reply: ReplyPresentation,
}

impl TrustPresentation {
    #[must_use]
    pub fn for_notification(notification: &unixnotis_core::NotificationView) -> Self {
        super::build::trust_presentation(notification)
    }
}

/// Identity content owned by a group or notification header
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentityPresentation {
    pub primary_label: String,
    pub secondary_claim: Option<String>,
    pub badge: BadgePresentation,
}

/// One daemon-approved application action
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionView {
    pub key: String,
    pub label: String,
}

/// Compact actions split without silently dropping safe overflow
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ActionPresentation {
    pub default_key: Option<String>,
    pub primary: Vec<ActionView>,
    pub overflow: Vec<ActionView>,
}

/// Whether a notification contains genuine bounded content media
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThumbnailKind {
    None,
    Content,
}

/// Shared media decision independent from GTK decoding
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MediaPresentation {
    pub thumbnail: ThumbnailKind,
}
