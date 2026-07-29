//! Card state classes and widget visibility

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
    let grouped = data.collapsed_group_preview || data.expanded;
    set_class_state(card, hooks::panel_card::GROUPED, grouped);
    set_class_state(card, hooks::panel_card::GROUP_FIRST, data.group_first);
    set_class_state(card, hooks::panel_card::GROUP_LAST, data.group_last);
    set_class_state(&row.card_plate, hooks::panel_card::GROUPED, grouped);
    set_class_state(
        &row.card_plate,
        hooks::panel_card::GROUP_FIRST,
        data.group_first,
    );
    set_class_state(
        &row.card_plate,
        hooks::panel_card::GROUP_LAST,
        data.group_last,
    );
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
    let grouped = data.collapsed_group_preview || data.expanded;
    if !grouped {
        return data.presentation.card_corners;
    }

    // The group header owns the top edge while only the final child owns bottom corners
    unixnotis_core::CutCorners {
        top_left: 0,
        top_right: 0,
        bottom_right: if data.group_last {
            data.presentation.card_corners.bottom_right
        } else {
            0
        },
        bottom_left: if data.group_last {
            data.presentation.card_corners.bottom_left
        } else {
            0
        },
    }
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
