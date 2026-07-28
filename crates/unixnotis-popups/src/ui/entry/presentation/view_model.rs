//! Bounded content and actions consumed by the kind-specific GTK builders

use std::time::{SystemTime, UNIX_EPOCH};

use unixnotis_core::{Action, NotificationView, Urgency};

use super::{PopupKind, PopupTrustPresentation};
use crate::ui::entry::labels::{
    clamp_label_text, has_visible_text, POPUP_ACTION_LABEL_MAX_CHARS, POPUP_APP_MAX_CHARS,
    POPUP_BODY_MAX_CHARS, POPUP_SUMMARY_MAX_CHARS,
};

const DECORATIVE_SQUARE_IMAGE_MAX: i32 = 128;

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
    pub(in crate::ui::entry) actions: Vec<ActionViewModel>,
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
        let actions = visible_actions(notification, kind);

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
            actions,
            trust,
            critical: notification.urgency == Urgency::Critical as u8,
        }
    }
}

fn visible_actions(notification: &NotificationView, kind: PopupKind) -> Vec<ActionViewModel> {
    // The daemon enforces the same boundary when a control client invokes an action
    if !notification.attribution.allows_application_actions() {
        return Vec::new();
    }

    notification
        .actions
        .iter()
        .filter(|action| action.key != "inline-reply")
        .take(kind.action_limit())
        .map(action_view_model)
        .collect()
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

    let badge = notification.attribution.badge_icon.trim();
    let source_matches_badge = !badge.is_empty()
        && (notification.image.icon_name.trim() == badge
            || notification.image.image_path.trim() == badge);
    let image_data = &notification.image.image_data;
    let looks_like_small_square_icon = notification.image.has_image_data
        && image_data.width > 0
        && image_data.width == image_data.height
        && image_data.width <= DECORATIVE_SQUARE_IMAGE_MAX;

    if source_matches_badge || looks_like_small_square_icon {
        ThumbnailKind::None
    } else {
        ThumbnailKind::Content
    }
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
