//! Notification metadata labels and relative timestamps

use std::time::{SystemTime, UNIX_EPOCH};

use unixnotis_core::{NotificationMetadataConfig, NotificationView, Urgency};
use unixnotis_ui::presentation::NotificationPresentation;

use super::super::super::super::item::RowData;
use super::super::state::NotificationRowWidgets;
use super::actions::visible_action_count_from;
use super::labels::{set_label_text_if_changed, set_label_visible_if_changed};
use super::visual::set_widget_visible_if_changed;

pub(super) fn update_metadata_labels(
    row: &NotificationRowWidgets,
    data: &RowData,
    notification: &NotificationView,
    presentation: &NotificationPresentation,
) {
    let metadata = data.presentation.metadata.as_ref();
    let time_badge = relative_time_badge(data.presentation.received_at_ms, metadata);
    // Keep the stock row compact unless the optional metadata lane is enabled
    set_label_visible_if_changed(
        &row.time_badge,
        data.presentation.show_metadata && !time_badge.is_empty(),
    );
    set_label_text_if_changed(&row.time_badge, &time_badge);
    set_widget_visible_if_changed(&row.footer, data.presentation.show_metadata);
    if !data.presentation.show_metadata {
        // Optional labels collapse together so ordinary rows match master spacing
        set_widget_visible_if_changed(&row.meta_top, false);
        set_label_visible_if_changed(&row.time_badge, false);
        set_label_visible_if_changed(&row.meta_label, false);
        set_label_visible_if_changed(&row.footer_left, false);
        set_label_visible_if_changed(&row.footer_right, false);
        return;
    }

    // Urgency copy comes from one config block so themes can rename every lane together
    let meta = notification_meta_label(notification, metadata);
    set_widget_visible_if_changed(&row.meta_top, !meta.is_empty());
    set_label_visible_if_changed(&row.meta_label, !meta.is_empty());
    set_label_text_if_changed(&row.meta_label, meta);

    // The left footer distinguishes live cards from retained history at a glance
    let footer_left = if notification.is_transient {
        metadata.transient_label.as_str()
    } else if data.is_active {
        metadata.live_label.as_str()
    } else {
        metadata.history_label.as_str()
    };
    set_label_visible_if_changed(&row.footer_left, !footer_left.is_empty());
    set_label_text_if_changed(&row.footer_left, footer_left);

    // Hidden reply actions are excluded from the displayed action count
    let action_count = visible_action_count_from(presentation, data.is_active);
    let footer_right = if action_count == 0 {
        String::new()
    } else if action_count == 1 {
        render_template(&metadata.action_count_one, "{count}", action_count)
    } else {
        render_template(&metadata.action_count_many, "{count}", action_count)
    };
    set_label_visible_if_changed(&row.footer_right, !footer_right.is_empty());
    set_label_text_if_changed(&row.footer_right, footer_right.as_ref());
}

pub(super) const fn notification_meta_label<'a>(
    notification: &NotificationView,
    metadata: &'a NotificationMetadataConfig,
) -> &'a str {
    // Unknown urgency values retain the normal notice presentation
    match notification.urgency {
        value if value == Urgency::Critical as u8 => metadata.critical_label.as_str(),
        value if value == Urgency::Low as u8 => metadata.low_label.as_str(),
        _ => metadata.normal_label.as_str(),
    }
}

pub(super) fn relative_time_badge(
    received_at_ms: i64,
    metadata: &NotificationMetadataConfig,
) -> String {
    if received_at_ms <= 0 {
        return String::new();
    }
    // A clock error should not prevent the row from rendering
    let Some(now_ms) = now_millis() else {
        return String::new();
    };
    relative_time_badge_at(received_at_ms, now_ms, metadata)
}

pub(super) fn relative_time_badge_at(
    received_at_ms: i64,
    now_ms: u128,
    metadata: &NotificationMetadataConfig,
) -> String {
    if received_at_ms <= 0 {
        return String::new();
    }
    // Saturation handles timestamps that are slightly ahead of the local clock
    let age_ms = now_ms.saturating_sub(received_at_ms.max(0) as u128);
    let age_secs = age_ms / 1_000;
    // Compact units keep the metadata lane from changing card width
    match age_secs {
        0..=59 => metadata.relative_now.clone(),
        60..=3_599 => render_template(&metadata.relative_minutes, "{value}", age_secs / 60),
        3_600..=86_399 => render_template(&metadata.relative_hours, "{value}", age_secs / 3_600),
        _ => render_template(&metadata.relative_days, "{value}", age_secs / 86_400),
    }
}

fn render_template(template: &str, token: &str, value: impl std::fmt::Display) -> String {
    // Missing tokens are allowed so a theme can use fixed copy for a whole bucket
    template.replace(token, &value.to_string())
}

fn now_millis() -> Option<u128> {
    // Systems with an invalid pre-epoch clock omit relative time safely
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_millis())
}
