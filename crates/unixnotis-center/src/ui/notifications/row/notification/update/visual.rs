//! Card state classes and widget visibility

use gtk::prelude::*;
use unixnotis_core::{hooks, NotificationView, Urgency};
use unixnotis_ui::presentation::{NotificationPresentation, TrustLevel};

use super::super::super::super::item::RowData;
use super::super::stack::stack_layer_visibility;
use super::super::state::NotificationRowWidgets;
use super::labels::has_visible_text;

pub(super) fn apply_visual_state(
    row: &NotificationRowWidgets,
    data: &RowData,
    notification: &NotificationView,
    has_actions: bool,
    has_thumbnail: bool,
) {
    let card = &row.card;
    let presentation = NotificationPresentation::from_view(notification);
    let is_critical = notification.urgency == Urgency::Critical as u8;
    // Theme changes update recycled rows without rebuilding the GTK child tree
    row.card_plate.set_corners(card_corners_for_row(data));
    // Explicit state updates prevent recycled rows from retaining stale classes
    set_class_state(card, hooks::shared_state::CRITICAL, is_critical);
    for (level, class_name) in [
        (TrustLevel::Verified, "verified"),
        (TrustLevel::Recognized, "recognized"),
        (TrustLevel::Unresolved, "unresolved"),
        (TrustLevel::Conflict, "conflict"),
        (TrustLevel::Relay, "relay"),
    ] {
        set_class_state(card, class_name, presentation.trust.level == level);
    }
    set_widget_visible_if_changed(&row.urgency_badge, is_critical);
    set_class_state(card, hooks::shared_state::ACTIVE, data.is_active);
    set_class_state(
        card,
        hooks::shared_state::COLLAPSED_GROUP_PREVIEW,
        data.collapsed_group_preview,
    );
    set_class_state(
        &row.card_plate,
        hooks::shared_state::COLLAPSED_GROUP_PREVIEW,
        data.collapsed_group_preview,
    );
    let layers = stack_layer_visibility(data.stack_depth);
    set_widget_visible_if_changed(&row.stack_middle, layers.middle);
    set_widget_visible_if_changed(&row.stack_back, layers.back);
    let grouped = data.collapsed_group_preview || data.expanded;
    set_class_state(card, hooks::panel_card::GROUPED, grouped);
    set_class_state(&row.card_plate, hooks::panel_card::GROUPED, grouped);
    set_class_state(
        card,
        hooks::panel_card::HAS_SUMMARY,
        has_visible_text(&notification.summary),
    );
    set_class_state(
        card,
        hooks::panel_card::HAS_BODY,
        has_visible_text(&notification.body),
    );
    set_class_state(card, hooks::panel_card::HAS_ACTIONS, has_actions);
    set_class_state(card, hooks::panel_card::NO_ACTIONS, !has_actions);
    set_class_state(card, hooks::panel_card::HAS_THUMBNAIL, has_thumbnail);
    set_class_state(card, hooks::panel_card::NO_THUMBNAIL, !has_thumbnail);
}

const fn card_corners_for_row(data: &RowData) -> unixnotis_core::CutCorners {
    // Every separated foreground card keeps the configured complete silhouette
    data.presentation.card_corners
}

fn set_class_state<W: IsA<gtk::Widget>>(root: &W, class_name: &str, enabled: bool) {
    // Guard CSS churn so GTK does not reprocess matching classes
    if enabled {
        if !root.has_css_class(class_name) {
            root.add_css_class(class_name);
        }
    } else if root.has_css_class(class_name) {
        root.remove_css_class(class_name);
    }
}

pub(super) fn set_widget_visible_if_changed<W: IsA<gtk::Widget>>(widget: &W, visible: bool) {
    // Stable visibility avoids unnecessary GTK property notifications
    if widget.get_visible() != visible {
        widget.set_visible(visible);
    }
}
