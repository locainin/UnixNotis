use std::collections::HashSet;

use super::super::window::refresh_popup_input_region;
use super::super::UiState;
use super::mutation::VisiblePopupUpdate;
use gtk::prelude::*;
use tracing::{debug, warn};
use unixnotis_ui::CutCorner;

use crate::dbus::UiCommand;
use crate::ui::entry::try_send_command;

impl UiState {
    pub(in super::super) fn update_popup_visibility(&mut self, force_region_refresh: bool) {
        // Visibility contract is driven strictly by configured max_visible count
        let max_visible = self.config.popups.max_visible;

        // Max-visible of zero disables popups entirely
        if max_visible == 0 {
            let update = self.apply_visible_popups(Vec::new());
            self.popup_window.set_visible(false);
            // Keep input region empty when popups are disabled
            if force_region_refresh || update.stack_changed {
                refresh_popup_input_region(
                    &self.popup_window,
                    &self.popup_stack,
                    &self.popup_input_region,
                );
            }
            debug!("popups disabled by max_visible = 0");
            return;
        }

        // Only the leading visible slice should pay GTK visibility churn on every update
        let desired_visible = self
            .popup_order
            .iter()
            .take(visible_popup_target(self.popup_order.len(), max_visible))
            .copied()
            .collect::<Vec<u32>>();
        let update = self.apply_visible_popups(desired_visible);
        // Window visibility follows the rows GTK actually represents, not just the
        // logical popup order that was requested upstream
        self.popup_window
            .set_visible(!self.visible_popups.is_empty());
        if force_region_refresh || update.stack_changed {
            refresh_popup_input_region(
                &self.popup_window,
                &self.popup_stack,
                &self.popup_input_region,
            );
        }
        debug!(
            visible = self.visible_popups.len(),
            total = self.popup_order.len(),
            "popup visibility updated"
        );
    }

    pub(in super::super) fn refresh_after_config_reload(&mut self) {
        // Only built rows have GTK wrappers that may need a corner refresh
        let materialized_roots = self
            .popups
            .values()
            .filter(|entry| entry.root.is_some())
            .count();
        for entry in self.popups.values() {
            let Some(root) = entry.root.as_ref() else {
                continue;
            };
            let Some(revealer) = entry.revealer.as_ref() else {
                continue;
            };
            let plate = revealer.child().and_downcast::<CutCorner>();
            match (self.config.theme.notification_corners.is_active(), plate) {
                (true, Some(plate)) => {
                    // Active cut geometry updates without rebuilding the full card
                    plate.set_corners(self.config.theme.notification_corners);
                }
                (true, None) => {
                    // Detach the ordinary card before moving it under the opt-in clipper
                    revealer.set_child(gtk::Widget::NONE);
                    let plate = CutCorner::new(root, self.config.theme.notification_corners);
                    revealer.set_child(Some(&plate));
                }
                (false, Some(plate)) => {
                    // Return the card to the revealer before dropping the disabled clipper
                    plate.set_child(gtk::Widget::NONE);
                    revealer.set_child(Some(root));
                }
                (false, None) => {}
            }
        }
        // Re-run visibility so max_visible changes take effect right away
        self.update_popup_visibility(true);
        debug!(
            materialized_roots,
            visible_target =
                visible_popup_target(self.popups.len(), self.config.popups.max_visible),
            total = self.popups.len(),
            "popup config reload refreshed"
        );
    }

    fn apply_visible_popups(&mut self, desired_visible: Vec<u32>) -> VisiblePopupUpdate {
        // Leaving the visible slice drops widget trees so overflow stays lightweight
        let previous_visible = self.visible_popups.clone();
        let desired_visible_set = desired_visible.iter().copied().collect::<HashSet<_>>();
        let restack_ids = visible_popup_restack_ids(&previous_visible, &desired_visible);
        let mut update = VisiblePopupUpdate::default();
        let mut applied_visible = Vec::with_capacity(desired_visible.len());
        let mut newly_materialized = Vec::new();
        for id in &previous_visible {
            if desired_visible_set.contains(id) {
                // Rows that stay visible keep their current widgets
                continue;
            }
            self.dematerialize_popup(*id);
            update.stack_changed = true;
        }

        // Attach or move only the rows that actually changed order
        let mut previous_revealer: Option<gtk::Revealer> = None;
        for id in &desired_visible {
            let was_materialized = self
                .popups
                .get(id)
                .is_some_and(super::super::entry::PopupEntry::is_materialized);
            self.materialize_popup(*id);
            let Some(entry) = self.popups.get(id) else {
                warn!(id, "popup marked visible but entry is missing");
                continue;
            };
            let Some(revealer) = entry.revealer.as_ref() else {
                warn!(id, "popup marked visible but revealer is missing");
                continue;
            };

            let needs_attach = revealer.parent().is_none();
            if needs_attach {
                // Freshly materialized rows enter the stack once before reveal state is flipped
                self.popup_stack.append(revealer);
                update.stack_changed = true;
            }

            if needs_attach || restack_ids.contains(id) {
                self.popup_stack
                    .reorder_child_after(revealer, previous_revealer.as_ref());
                update.stack_changed = true;
            }

            previous_revealer = Some(revealer.clone());
            if !was_materialized {
                // Materialization is complete only after the row joins the live stack
                try_send_command(
                    &self.command_tx,
                    UiCommand::Materialized(entry.notification.key()),
                );
                newly_materialized.push(entry.notification.key());
            }
            applied_visible.push(*id);
        }

        // Visible rows get their final reveal state after the stack order is correct
        for id in &applied_visible {
            let Some(entry) = self.popups.get(id) else {
                continue;
            };
            let (Some(root), Some(revealer)) = (entry.root.as_ref(), entry.revealer.as_ref())
            else {
                continue;
            };
            if !root.is_visible() {
                root.set_visible(true);
                update.stack_changed = true;
            }
            if !revealer.reveals_child() {
                revealer.set_reveal_child(true);
                update.stack_changed = true;
            }
            if !root.has_css_class("unixnotis-popup-visible") {
                root.add_css_class("unixnotis-popup-visible");
            }
        }

        self.visible_popups = applied_visible;
        for key in newly_materialized {
            self.schedule_popup_hide(key);
        }
        update
    }
}

pub(super) fn visible_popup_target(total_popups: usize, max_visible: usize) -> usize {
    // Visible slice can never exceed the number of known popups
    total_popups.min(max_visible)
}

pub(super) fn visible_popup_restack_ids(
    previous_visible: &[u32],
    desired_visible: &[u32],
) -> HashSet<u32> {
    // Work on a local order copy so move decisions stay deterministic and testable
    let mut working = previous_visible.to_vec();
    let mut moved = HashSet::new();

    for (target_index, id) in desired_visible.iter().copied().enumerate() {
        let Some(current_index) = working.iter().position(|current| *current == id) else {
            // New ids need one attach at their final slot
            working.insert(target_index, id);
            moved.insert(id);
            continue;
        };

        if current_index == target_index {
            // Rows already in the right slot do not need another GTK reorder
            continue;
        }

        // Move only the row that is actually out of place
        let moved_id = working.remove(current_index);
        working.insert(target_index, moved_id);
        moved.insert(id);
    }

    // Return only rows that need a GTK attach or reorder call
    moved
}

#[cfg(test)]
#[path = "tests/visibility.rs"]
mod tests;
