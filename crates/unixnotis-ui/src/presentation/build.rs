//! Derivation of one shared notification presentation snapshot

use std::time::{SystemTime, UNIX_EPOCH};

use unixnotis_core::{
    Action, ApplicationActionPolicy, AttributionStatus, InlineReplyPolicy, NotificationView,
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
        let kind = notification_kind(notification);
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
    match decision.admission_at_commit {
        PopupAdmissionView::Rule => {
            return Some("Not shown — matched a notification rule".to_string());
        }
        PopupAdmissionView::Dnd => {
            return Some("Not shown — Do Not Disturb was enabled".to_string());
        }
        PopupAdmissionView::Inhibitor => {
            return Some("Not shown — notifications were inhibited".to_string());
        }
        PopupAdmissionView::RendererDisabled => {
            return Some("Not shown — popups are disabled".to_string());
        }
        PopupAdmissionView::RendererUnavailable => {
            if decision.delivery_stage != unixnotis_core::PopupDeliveryStage::Visible {
                return Some("Not shown — popup renderer was unavailable".to_string());
            }
        }
        PopupAdmissionView::Show => {}
    }

    matches!(
        decision.delivery_stage,
        unixnotis_core::PopupDeliveryStage::FanoutFailed
    )
    .then(|| "Not shown — live notification delivery failed".to_string())
}

pub(super) fn trust_presentation(notification: &NotificationView) -> TrustPresentation {
    let level = trust_level(notification);
    let short_label = match level {
        // Verified and relay primary labels already communicate their source clearly
        TrustLevel::Verified | TrustLevel::Relay => None,
        TrustLevel::Recognized | TrustLevel::Unresolved => Some("Unverified".to_string()),
        TrustLevel::Conflict => Some("Suspicious".to_string()),
    };
    let details_label = nonempty_text(&notification.attribution.diagnostic_detail);
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
    match notification.attribution.status {
        AttributionStatus::Verified => TrustLevel::Verified,
        AttributionStatus::Recognized => TrustLevel::Recognized,
        AttributionStatus::Unresolved => TrustLevel::Unresolved,
        AttributionStatus::Conflict => TrustLevel::Conflict,
        AttributionStatus::Relay => TrustLevel::Relay,
    }
}

fn identity_presentation(
    notification: &NotificationView,
    level: TrustLevel,
) -> IdentityPresentation {
    let display_name =
        clamp_label_text(&notification.attribution.display_name, APP_LABEL_MAX_CHARS);
    let claimed_name =
        clamp_label_text(&notification.attribution.claimed_name, APP_LABEL_MAX_CHARS);
    let (primary_label, secondary_claim) = match notification.attribution.status {
        AttributionStatus::Verified | AttributionStatus::Recognized => (
            display_name.into_owned(),
            differing_claim(&notification.attribution.display_name, &claimed_name),
        ),
        AttributionStatus::Relay => (
            "Command-line notification".to_string(),
            visible_claim(&claimed_name).map(|claim| format!("App label: {claim}")),
        ),
        AttributionStatus::Conflict => (
            "Unknown application".to_string(),
            visible_claim(&claimed_name).map(|claim| format!("Claimed app: {claim}")),
        ),
        AttributionStatus::Unresolved => (
            "Unknown application".to_string(),
            visible_claim(&claimed_name).map(|claim| format!("App label: {claim}")),
        ),
    };
    let badge = match level {
        TrustLevel::Verified => BadgePresentation::AuthenticatedApplication,
        TrustLevel::Recognized => BadgePresentation::RecognizedApplication,
        TrustLevel::Unresolved => BadgePresentation::UnknownApplication,
        TrustLevel::Conflict => BadgePresentation::SuspiciousApplication,
        TrustLevel::Relay => BadgePresentation::CommandLine,
    };
    IdentityPresentation {
        primary_label,
        secondary_claim,
        badge,
    }
}

fn differing_claim(display_name: &str, claimed_name: &str) -> Option<String> {
    let claim = visible_claim(claimed_name)?;
    (!claim.eq_ignore_ascii_case(display_name.trim())).then(|| format!("App label: {claim}"))
}

fn visible_claim(claim: &str) -> Option<&str> {
    let claim = claim.trim();
    (!claim.is_empty() && claim != "Unknown application").then_some(claim)
}

pub(super) fn notification_kind(notification: &NotificationView) -> NotificationKind {
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
    } else if media_category_class(category_class) {
        NotificationKind::Media
    } else {
        NotificationKind::Utility
    }
}

fn media_category_class(category_class: &str) -> bool {
    ["image", "media", "photo", "video", "audio"]
        .iter()
        .any(|candidate| category_class.eq_ignore_ascii_case(candidate))
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
    let identity_is_verified =
        matches!(notification.attribution.status, AttributionStatus::Verified);
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
