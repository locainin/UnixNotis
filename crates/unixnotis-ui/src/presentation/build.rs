//! Derivation of one shared notification presentation snapshot

use std::time::{SystemTime, UNIX_EPOCH};

use unixnotis_core::{
    Action, ApplicationActionPolicy, AttributionClass, InlineReplyPolicy, NotificationView,
    PopupAdmissionView, Urgency,
};

use super::text::{
    clamp_label_text, has_visible_text, ACTION_LABEL_MAX_CHARS, APP_LABEL_MAX_CHARS,
    BODY_LABEL_MAX_CHARS, SUMMARY_LABEL_MAX_CHARS,
};
use super::types::{
    ActionPresentation, ActionView, BadgePresentation, IdentityPresentation, MediaPresentation,
    NotificationKind, ReplyPresentation, ThumbnailKind, TrustLevel, TrustPresentation,
};

/// Complete non-GTK notification presentation shared by popup and panel adapters
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotificationPresentation {
    pub kind: NotificationKind,
    pub trust: TrustPresentation,
    pub identity: IdentityPresentation,
    pub title: String,
    pub body: Option<String>,
    pub timestamp: String,
    pub popup_status: Option<String>,
    pub media: MediaPresentation,
    pub actions: ActionPresentation,
    pub critical: bool,
}

impl NotificationPresentation {
    #[must_use]
    pub fn from_view(notification: &NotificationView) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| {
                i64::try_from(duration.as_secs()).unwrap_or(i64::MAX)
            });
        Self::from_view_at(notification, now)
    }

    #[must_use]
    pub fn from_view_at(notification: &NotificationView, now: i64) -> Self {
        let trust = trust_presentation(notification);
        let kind = notification_kind(notification, trust.level);
        let identity = identity_presentation(notification, trust.level);

        Self {
            kind,
            trust,
            identity,
            title: clamp_label_text(&notification.summary, SUMMARY_LABEL_MAX_CHARS).into_owned(),
            body: has_visible_text(&notification.body)
                .then(|| clamp_label_text(&notification.body, BODY_LABEL_MAX_CHARS).into_owned()),
            timestamp: relative_time_label(notification.received_at_unix_seconds, now),
            popup_status: popup_status(notification),
            media: MediaPresentation {
                thumbnail: thumbnail_kind(notification),
            },
            actions: visible_actions(notification, kind),
            critical: notification.urgency == Urgency::Critical as u8,
        }
    }
}

fn popup_status(notification: &NotificationView) -> Option<String> {
    let decision = &notification.popup_decision;
    if decision.decided_at_unix_ms <= 0 {
        return None;
    }
    if decision.delivery_stage == unixnotis_core::PopupDeliveryStage::Rendered {
        return (decision.admission_at_commit == PopupAdmissionView::RendererUnavailable)
            .then(|| "Shown after popup renderer recovered".to_string());
    }
    let status = match decision.delivery_stage {
        unixnotis_core::PopupDeliveryStage::FanoutFailed => {
            "Not shown — notification delivery failed"
        }
        _ => match decision.admission_at_commit {
            PopupAdmissionView::Show => return None,
            PopupAdmissionView::Rule => "Not shown — matched notification rule",
            PopupAdmissionView::Dnd => "Not shown — Do Not Disturb was enabled",
            PopupAdmissionView::Inhibitor => "Not shown — notifications were inhibited",
            PopupAdmissionView::RendererUnavailable => "Not shown — popup renderer was unavailable",
            PopupAdmissionView::RendererDisabled => "Not shown — popups are disabled",
        },
    };
    Some(status.to_string())
}

pub(super) fn trust_presentation(notification: &NotificationView) -> TrustPresentation {
    let level = trust_level(notification);
    let short_label = match level {
        // Verified and command-line primary labels already communicate their source clearly
        TrustLevel::Verified | TrustLevel::CommandLine => None,
        TrustLevel::Unverified => Some("Unverified".to_string()),
        TrustLevel::Suspicious => Some("Suspicious".to_string()),
    };
    let details_label = nonempty_text(&notification.attribution.source_label);
    let has_reply_action = notification
        .actions
        .iter()
        .any(|action| action.key == "inline-reply");
    let has_reply_request = notification.inline_reply.available || has_reply_action;
    let reply = if has_reply_action
        && notification.inline_reply.available
        && notification.inline_reply_policy == InlineReplyPolicy::Allow
        && level == TrustLevel::Verified
    {
        ReplyPresentation::Available
    } else if has_reply_request {
        ReplyPresentation::Unavailable
    } else {
        ReplyPresentation::Hidden
    };

    TrustPresentation {
        level,
        short_label,
        details_label,
        reply,
    }
}

const fn trust_level(notification: &NotificationView) -> TrustLevel {
    match notification.attribution.class {
        AttributionClass::SystemAssociated | AttributionClass::PortalAssociated => {
            if notification.attribution.has_warning() {
                TrustLevel::Suspicious
            } else {
                TrustLevel::Verified
            }
        }
        AttributionClass::UserAssociated | AttributionClass::Unknown => {
            if notification.attribution.has_warning() {
                TrustLevel::Suspicious
            } else {
                TrustLevel::Unverified
            }
        }
        // A verified relay remains a relay even when its caller-controlled label names an app
        AttributionClass::TrustedRelay => TrustLevel::CommandLine,
        AttributionClass::Conflict => TrustLevel::Suspicious,
    }
}

fn identity_presentation(
    notification: &NotificationView,
    level: TrustLevel,
) -> IdentityPresentation {
    let claimed_label =
        clamp_label_text(&notification.attribution.display_name, APP_LABEL_MAX_CHARS);
    let (primary_label, secondary_claim) = match notification.attribution.class {
        AttributionClass::TrustedRelay => (
            "Command-line notification".to_string(),
            visible_claim(&claimed_label).map(|claim| format!("App label: {claim}")),
        ),
        AttributionClass::Conflict => (
            "Unknown application".to_string(),
            claimed_identity(&notification.attribution.source_label)
                .map(|claim| format!("Claims “{claim}”")),
        ),
        AttributionClass::Unknown => (
            "Unknown application".to_string(),
            visible_claim(&claimed_label).map(|claim| format!("App label: {claim}")),
        ),
        AttributionClass::SystemAssociated
        | AttributionClass::PortalAssociated
        | AttributionClass::UserAssociated => (claimed_label.into_owned(), None),
    };
    let badge = match level {
        TrustLevel::Verified => BadgePresentation::AuthenticatedApplication,
        TrustLevel::Unverified => BadgePresentation::UnknownApplication,
        TrustLevel::Suspicious => BadgePresentation::SuspiciousApplication,
        TrustLevel::CommandLine => BadgePresentation::CommandLine,
    };
    IdentityPresentation {
        primary_label,
        secondary_claim,
        badge,
    }
}

fn claimed_identity(source: &str) -> Option<String> {
    let claim = source
        .split(';')
        .next()
        .map(str::trim)
        .and_then(|value| value.strip_prefix("Claims to be "))?
        .trim();
    visible_claim(claim).map(ToString::to_string)
}

fn visible_claim(claim: &str) -> Option<&str> {
    let claim = claim.trim();
    (!claim.is_empty() && claim != "Unknown application").then_some(claim)
}

pub(super) fn notification_kind(
    notification: &NotificationView,
    trust_level: TrustLevel,
) -> NotificationKind {
    match trust_level {
        TrustLevel::Suspicious => return NotificationKind::Warning,
        TrustLevel::Unverified | TrustLevel::CommandLine => return NotificationKind::Utility,
        TrustLevel::Verified => {}
    }
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
        NotificationKind::Communication
    } else {
        NotificationKind::Utility
    }
}

fn communication_category_class(category_class: &str) -> bool {
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

fn visible_actions(notification: &NotificationView, kind: NotificationKind) -> ActionPresentation {
    if notification.attribution.application_action_policy() != ApplicationActionPolicy::Allow {
        return ActionPresentation::default();
    }
    let mut actions = notification
        .actions
        .iter()
        .filter(|action| action.key != "inline-reply")
        .map(action_view)
        .collect::<Vec<_>>();
    let overflow = actions.split_off(actions.len().min(kind.action_limit()));
    ActionPresentation {
        primary: actions,
        overflow,
    }
}

fn action_view(action: &Action) -> ActionView {
    ActionView {
        key: action.key.clone(),
        label: clamp_label_text(&action.label, ACTION_LABEL_MAX_CHARS).into_owned(),
    }
}

fn thumbnail_kind(notification: &NotificationView) -> ThumbnailKind {
    let has_content =
        notification.image.has_image_data || !notification.image.image_path.trim().is_empty();
    if !has_content {
        return ThumbnailKind::None;
    }
    let category_is_media = ["image", "media", "photo"].iter().any(|category| {
        notification
            .category
            .split('.')
            .next()
            .unwrap_or_default()
            .eq_ignore_ascii_case(category)
    });
    let identity_is_verified = matches!(
        notification.attribution.class,
        AttributionClass::SystemAssociated | AttributionClass::PortalAssociated
    ) && !notification.attribution.has_warning();
    if !identity_is_verified {
        // Untrusted senders need an explicit media category before large imagery is shown
        return if category_is_media {
            ThumbnailKind::Content
        } else {
            ThumbnailKind::None
        };
    }
    if notification.image.has_image_data
        || category_is_media
        || !image_path_matches_authenticated_badge(notification)
    {
        return ThumbnailKind::Content;
    }
    ThumbnailKind::None
}

fn image_path_matches_authenticated_badge(notification: &NotificationView) -> bool {
    let badge = notification.attribution.badge_icon.trim();
    if badge.is_empty() {
        return false;
    }
    let image_path = notification.image.image_path.trim();
    if image_path.is_empty() {
        return false;
    }
    if image_path == badge {
        return true;
    }

    // Canonical identity handles symlink aliases without treating dimensions as evidence
    let badge_path = std::path::Path::new(badge);
    let image_path = std::path::Path::new(image_path);
    if !badge_path.is_absolute() || !image_path.is_absolute() {
        return false;
    }
    let Some(badge_path) = std::fs::canonicalize(badge_path).ok() else {
        return false;
    };
    std::fs::canonicalize(image_path).is_ok_and(|path| path == badge_path)
}

fn relative_time_label(received_at: i64, now: i64) -> String {
    if received_at <= 0 {
        return "now".to_string();
    }
    let age = now.saturating_sub(received_at).max(0);
    match age {
        0..=59 => "now".to_string(),
        60..=3_599 => format!("{}m", age / 60),
        3_600..=86_399 => format!("{}h", age / 3_600),
        _ => format!("{}d", age / 86_400),
    }
}

fn nonempty_text(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}
