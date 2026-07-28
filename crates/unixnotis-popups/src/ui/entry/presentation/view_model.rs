//! Popup adapter over the shared non-GTK notification presentation

use std::time::{SystemTime, UNIX_EPOCH};

use unixnotis_core::NotificationView;
use unixnotis_ui::presentation::NotificationPresentation;

use super::{PopupKind, PopupTrustPresentation};

pub(in crate::ui::entry) use unixnotis_ui::presentation::{
    ActionView as ActionViewModel, ThumbnailKind,
};

/// Popup field names retained as a thin adapter for the kind-specific GTK builders
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::ui::entry) struct PopupEntryViewModel {
    pub(in crate::ui::entry) kind: PopupKind,
    pub(in crate::ui::entry) app_label: String,
    pub(in crate::ui::entry) secondary_claim: Option<String>,
    pub(in crate::ui::entry) badge: unixnotis_ui::presentation::BadgePresentation,
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
        Self::from_shared(NotificationPresentation::from_view_at(notification, now))
    }

    fn from_shared(shared: NotificationPresentation) -> Self {
        Self {
            kind: shared.kind,
            app_label: shared.identity.primary_label,
            secondary_claim: shared.identity.secondary_claim,
            badge: shared.identity.badge,
            timestamp_label: shared.timestamp,
            title: shared.title,
            body: shared.body,
            thumbnail: shared.media.thumbnail,
            primary_actions: shared.actions.primary,
            overflow_actions: shared.actions.overflow,
            trust: shared.trust,
            critical: shared.critical,
        }
    }
}
