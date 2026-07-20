//! Card classes, stack depth, and widget visibility

use gtk::prelude::*;
use unixnotis_core::{hooks, NotificationView, Urgency};

use super::super::super::super::item::RowData;
use super::super::state::NotificationRowWidgets;
use super::labels::has_visible_text;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct StackGhostVisibility {
    pub(super) middle: bool,
    pub(super) back: bool,
}

pub(super) const fn stack_ghost_visibility(stack_depth: u8) -> StackGhostVisibility {
    // A single rear layer uses the back slot because it starts without overlap
    StackGhostVisibility {
        middle: stack_depth >= 2,
        back: stack_depth >= 1,
    }
}

pub(super) fn apply_visual_state(
    row: &NotificationRowWidgets,
    data: &RowData,
    notification: &NotificationView,
    has_actions: bool,
    has_thumbnail: bool,
) {
    let card = &row.card;
    // Theme changes update recycled rows without rebuilding the GTK child tree
    row.card_plate.set_corners(data.presentation.card_corners);
    // Explicit state updates prevent recycled rows from retaining stale classes
    set_class_state(
        card,
        hooks::shared_state::CRITICAL,
        notification.urgency == Urgency::Critical as u8,
    );
    set_class_state(card, hooks::shared_state::ACTIVE, data.is_active);
    set_class_state(card, hooks::shared_state::STACKED, data.stacked);
    set_class_state(card, hooks::panel_card::GROUPED, true);
    set_class_state(card, hooks::panel_card::GROUP_COLLAPSED, data.stacked);
    set_class_state(card, hooks::panel_card::GROUP_EXPANDED, data.expanded);

    // Rear layers occupy fixed paint slots with different overlap rules
    let ghost_visibility = stack_ghost_visibility(data.stack_depth);
    set_widget_visible_if_changed(&row.stack_ghost_1, ghost_visibility.middle);
    set_widget_visible_if_changed(&row.stack_ghost_2, ghost_visibility.back);

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
