//! Bounded content and actions consumed by the kind-specific GTK builders

use std::time::{SystemTime, UNIX_EPOCH};

use unixnotis_core::{Action, ApplicationActionPolicy, NotificationView, Urgency};

use super::{PopupKind, PopupTrustPresentation};
use crate::ui::entry::labels::{
    clamp_label_text, has_visible_text, POPUP_ACTION_LABEL_MAX_CHARS, POPUP_APP_MAX_CHARS,
    POPUP_BODY_MAX_CHARS, POPUP_SUMMARY_MAX_CHARS,
};

/// One safe application action prepared for a compact popup button
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::ui::entry) struct ActionViewModel {
    pub(in crate::ui::entry) key: String,
    pub(in crate::ui::entry) label: String,
}

/// Whether the payload contains a genuine content image worth showing
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::ui::entry) enum ThumbnailKind {
    None,
    Content,
}

/// Presentation data kept separate from raw attribution evidence
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::ui::entry) struct PopupEntryViewModel {
    pub(in crate::ui::entry) kind: PopupKind,
    pub(in crate::ui::entry) app_label: String,
    pub(in crate::ui::entry) timestamp_label: String,
    pub(in crate::ui::entry) title: String,
    pub(in crate::ui::entry) body: Option<String>,
    pub(in crate::ui::entry) thumbnail: ThumbnailKind,
    pub(in crate::ui::entry) primary_actions: Vec<ActionViewModel>,
    pub(in crate::ui::entry) overflow_actions: Vec<ActionViewModel>,
    pub(in crate::ui::entry) trust: PopupTrustPresentation,
    pub(in crate::ui::entry) critical: bool,
}

impl PopupEntryViewModel {
    pub(in crate::ui::entry) fn for_notification(notification: &NotificationView) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| {
                i64::try_from(duration.as_secs()).unwrap_or(i64::MAX)
            });
        Self::for_notification_at(notification, now)
    }

    pub(in crate::ui::entry) fn for_notification_at(
        notification: &NotificationView,
        now: i64,
    ) -> Self {
        let trust = PopupTrustPresentation::for_notification(notification);
        let kind = PopupKind::for_notification(notification, trust.level);
        let (primary_actions, overflow_actions) = visible_actions(notification, kind);

        Self {
            kind,
            app_label: clamp_label_text(
                &notification.attribution.display_name,
                POPUP_APP_MAX_CHARS,
            )
            .into_owned(),
            timestamp_label: relative_time_label(notification.received_at_unix_seconds, now),
            title: clamp_label_text(&notification.summary, POPUP_SUMMARY_MAX_CHARS).into_owned(),
            body: has_visible_text(&notification.body)
                .then(|| clamp_label_text(&notification.body, POPUP_BODY_MAX_CHARS).into_owned()),
            thumbnail: thumbnail_kind(notification),
            primary_actions,
            overflow_actions,
            trust,
            critical: notification.urgency == Urgency::Critical as u8,
        }
    }
}

fn visible_actions(
    notification: &NotificationView,
    kind: PopupKind,
) -> (Vec<ActionViewModel>, Vec<ActionViewModel>) {
    // The daemon enforces the same boundary when a control client invokes an action
    if notification.attribution.application_action_policy() != ApplicationActionPolicy::Allow {
        return (Vec::new(), Vec::new());
    }

    let mut actions = notification
        .actions
        .iter()
        .filter(|action| action.key != "inline-reply")
        .map(action_view_model)
        .collect::<Vec<_>>();
    let overflow_actions = actions.split_off(actions.len().min(kind.action_limit()));
    (actions, overflow_actions)
}

fn action_view_model(action: &Action) -> ActionViewModel {
    ActionViewModel {
        key: action.key.clone(),
        label: clamp_label_text(&action.label, POPUP_ACTION_LABEL_MAX_CHARS).into_owned(),
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
    if category_is_media {
        return ThumbnailKind::Content;
    }

    if image_source_matches_authenticated_badge(notification) {
        ThumbnailKind::None
    } else {
        ThumbnailKind::Content
    }
}

fn image_source_matches_authenticated_badge(notification: &NotificationView) -> bool {
    let badge = notification.attribution.badge_icon.trim();
    if badge.is_empty() {
        return false;
    }
    if notification.image.icon_name.trim() == badge {
        return true;
    }

    let image_path = notification.image.image_path.trim();
    if image_path.is_empty() {
        return false;
    }
    if image_path == badge {
        return true;
    }

    // Canonical file identity handles symlink aliases without guessing from dimensions
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
    // Missing timestamps cannot produce a meaningful age
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
