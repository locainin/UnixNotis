//! Card classes, stack depth, and widget visibility

use gtk::prelude::*;
use unixnotis_core::{hooks, NotificationView, Urgency};
use unixnotis_ui::presentation::{NotificationPresentation, TrustLevel};

use super::super::super::super::item::RowData;
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
    row.card_plate.set_corners(data.presentation.card_corners);
    // Explicit state updates prevent recycled rows from retaining stale classes
    set_class_state(card, hooks::shared_state::CRITICAL, is_critical);
    for (level, class_name) in [
        (TrustLevel::Verified, "verified"),
        (TrustLevel::Unverified, "unverified"),
        (TrustLevel::Suspicious, "suspicious"),
        (TrustLevel::System, "system"),
    ] {
        set_class_state(card, class_name, presentation.trust.level == level);
    }
    set_widget_visible_if_changed(&row.urgency_badge, is_critical);
    set_class_state(card, hooks::shared_state::ACTIVE, data.is_active);
    set_class_state(card, hooks::shared_state::STACKED, data.stacked);
    let grouped = data.stacked || data.expanded;
    set_class_state(card, hooks::panel_card::GROUPED, grouped);
    set_class_state(card, hooks::panel_card::GROUP_COLLAPSED, data.stacked);
    set_class_state(card, hooks::panel_card::GROUP_EXPANDED, data.expanded);

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

fn set_class_state(root: &gtk::Box, class_name: &str, enabled: bool) {
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
