//! Notification metadata labels and relative timestamps

use std::borrow::Cow;
use std::time::{SystemTime, UNIX_EPOCH};

use unixnotis_core::{NotificationView, Urgency};

use super::super::super::super::item::RowData;
use super::super::state::NotificationRowWidgets;
use super::actions::visible_action_count;
use super::labels::{set_label_text_if_changed, set_label_visible_if_changed};
use super::visual::set_widget_visible_if_changed;

pub(super) fn update_metadata_labels(
    row: &NotificationRowWidgets,
    data: &RowData,
    notification: &NotificationView,
) {
    // Metadata visibility controls both the compact header and footer lanes
    set_widget_visible_if_changed(&row.meta_top, data.presentation.show_metadata);
    set_widget_visible_if_changed(&row.footer, data.presentation.show_metadata);
    if !data.presentation.show_metadata {
        // Disabled lanes collapse fully so compact cards retain their shape
        set_label_visible_if_changed(&row.meta_label, false);
        set_label_visible_if_changed(&row.time_badge, false);
        set_label_visible_if_changed(&row.footer_left, false);
        set_label_visible_if_changed(&row.footer_right, false);
        return;
    }

    // Urgency uses short stable labels that remain useful across themes
    let meta = notification_meta_label(notification);
    set_label_visible_if_changed(&row.meta_label, true);
    set_label_text_if_changed(&row.meta_label, &meta);

    // Missing or invalid timestamps hide the badge instead of showing stale text
    let time_badge = relative_time_badge(data.presentation.received_at_ms);
    set_label_visible_if_changed(&row.time_badge, !time_badge.is_empty());
    set_label_text_if_changed(&row.time_badge, &time_badge);

    // The left footer distinguishes live cards from retained history at a glance
    let footer_left = if notification.is_transient {
        "TRANSIENT"
    } else if data.is_active {
        "LIVE"
    } else {
        "HISTORY"
    };
    set_label_visible_if_changed(&row.footer_left, true);
    set_label_text_if_changed(&row.footer_left, footer_left);

    // Hidden reply actions are excluded from the displayed action count
    let action_count = visible_action_count(notification, data.is_active);
    let footer_right = if action_count == 0 {
        Cow::Borrowed("")
    } else {
        Cow::Owned(format!("{action_count} ACTIONS"))
    };
    set_label_visible_if_changed(&row.footer_right, !footer_right.is_empty());
    set_label_text_if_changed(&row.footer_right, footer_right.as_ref());
}

pub(super) fn notification_meta_label(notification: &NotificationView) -> String {
    // Unknown urgency values retain the normal notice presentation
    match notification.urgency {
        value if value == Urgency::Critical as u8 => "ALERT".to_string(),
        value if value == Urgency::Low as u8 => "LOW".to_string(),
        _ => "NOTICE".to_string(),
    }
}

pub(super) fn relative_time_badge(received_at_ms: i64) -> String {
    if received_at_ms <= 0 {
        return String::new();
    }
    // A clock error should not prevent the row from rendering
    let Some(now_ms) = now_millis() else {
        return String::new();
    };
    // Saturation handles timestamps that are slightly ahead of the local clock
    let age_ms = now_ms.saturating_sub(received_at_ms.max(0) as u128);
    let age_secs = age_ms / 1_000;
    // Compact units keep the metadata lane from changing card width
    match age_secs {
        0..=59 => "now".to_string(),
        60..=3_599 => format!("{}m", age_secs / 60),
        3_600..=86_399 => format!("{}h", age_secs / 3_600),
        _ => format!("{}d", age_secs / 86_400),
    }
}

fn now_millis() -> Option<u128> {
    // Systems with an invalid pre-epoch clock omit relative time safely
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_millis())
}
