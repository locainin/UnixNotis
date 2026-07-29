//! Popup list mutation and materialization

use gtk::prelude::*;
use tracing::debug;
use unixnotis_core::{NotificationKey, NotificationView};
use unixnotis_ui::CutCorner;

use super::super::entry::{try_send_command, PopupEntry};
use super::super::window::refresh_popup_input_region;
use super::super::UiState;
use crate::dbus::UiCommand;

pub(super) struct ReconcilePlan {
    // Local rows missing from the daemon snapshot
    pub(super) stale_ids: Vec<u32>,
    // Rows that must be inserted or updated to match daemon truth
    pub(super) updates: Vec<NotificationView>,
    // Final order copied from the daemon seed
    pub(super) desired_order: std::collections::VecDeque<u32>,
}

pub(super) fn incoming_generation_is_stale(existing: Option<u64>, incoming: u64) -> bool {
    existing.is_some_and(|generation| generation > incoming)
}

pub(super) fn generation_matches(existing: Option<u64>, expected: u64) -> bool {
    existing.is_some_and(|generation| generation == expected)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct VisiblePopupUpdate {
    // True when stack order, materialization, or reveal state changed
    pub(super) stack_changed: bool,
}

impl UiState {
    pub(in crate::ui) fn add_popup(&mut self, notification: NotificationView) {
        // Runtime insert path keeps one place for add semantics
        self.add_popup_internal(notification, true);
    }

    pub(super) fn add_popup_internal(
        &mut self,
        notification: NotificationView,
        refresh_visibility: bool,
    ) {
        let id = notification.id;
        if let Some(existing) = self.popups.get(&id) {
            // A later generation always dominates an old or duplicated add event
            if existing.notification.generation >= notification.generation {
                debug!(id, "stale popup insert skipped");
                return;
            }
            self.update_popup_internal(notification, true, refresh_visibility);
            return;
        }

        // Hidden overflow rows stay as plain data until they can actually be shown
        self.popups.insert(id, PopupEntry::queued(notification));
        self.popup_order.push_front(id);
        if refresh_visibility {
            self.update_popup_visibility(false);
        }
        debug!(id, total = self.popup_order.len(), "popup inserted");
    }

    pub(in crate::ui) fn update_popup(&mut self, notification: NotificationView, show_popup: bool) {
        // Update path can also hide a popup when policy says not to show it
        self.update_popup_internal(notification, show_popup, true);
    }

    pub(super) fn update_popup_internal(
        &mut self,
        notification: NotificationView,
        show_popup: bool,
        refresh_visibility: bool,
    ) -> bool {
        let id = notification.id;
        let existing_generation = self
            .popups
            .get(&id)
            .map(|entry| entry.notification.generation);
        if incoming_generation_is_stale(existing_generation, notification.generation) {
            // Reordered older updates cannot roll a popup back
            debug!(
                id,
                generation = notification.generation,
                "stale popup update skipped"
            );
            return false;
        }
        if !show_popup {
            // A newer suppressed generation removes any older visible payload for this ID
            self.remove_popup_internal(id, refresh_visibility);
            return false;
        }

        if !self.popups.contains_key(&id) {
            // Same helper handles late updates for ids that were not present locally
            self.add_popup_internal(notification, refresh_visibility);
            return false;
        }

        let rebuilt_visible_row = if self
            .popups
            .get(&id)
            .is_some_and(PopupEntry::is_materialized)
        {
            self.rebuild_materialized_popup(&notification)
        } else {
            false
        };

        if let Some(entry) = self.popups.get_mut(&id) {
            // Cached payload stays in sync with the rebuilt or queued row
            entry.notification = notification;
        }

        if refresh_visibility {
            self.update_popup_visibility(rebuilt_visible_row);
        }
        debug!(id, "popup updated");
        rebuilt_visible_row
    }

    pub(in crate::ui) fn remove_popup_if_generation(&mut self, key: NotificationKey) {
        let existing_generation = self
            .popups
            .get(&key.id)
            .map(|entry| entry.notification.generation);
        if generation_matches(existing_generation, key.generation) {
            self.remove_popup_internal(key.id, true);
        }
    }

    pub(super) fn remove_popup_internal(&mut self, id: u32, refresh_visibility: bool) {
        if let Some(entry) = self.popups.remove(&id) {
            if let Some(revealer) = entry.revealer {
                // Visible rows animate out before leaving the stack
                revealer.set_reveal_child(false);
                let stack = self.popup_stack.clone();
                let popup_window = self.popup_window.clone();
                let popup_input_region = self.popup_input_region.clone();
                revealer.connect_notify_local(Some("child-revealed"), move |revealer, _| {
                    // Remove only after transition completes to avoid visual pop
                    if !revealer.is_child_revealed() && revealer.parent().is_some() {
                        stack.remove(revealer);
                    }
                    // Re-sync clickable shape after each reveal step
                    refresh_popup_input_region(&popup_window, &stack, &popup_input_region);
                });
            }
        }
        self.popup_order.retain(|item| *item != id);
        if refresh_visibility {
            self.update_popup_visibility(false);
        }
        debug!(id, total = self.popup_order.len(), "popup removed");
    }

    fn rebuild_materialized_popup(&mut self, notification: &NotificationView) -> bool {
        let id = notification.id;
        let Some(revealer) = self
            .popups
            .get(&id)
            .and_then(|entry| entry.revealer.clone())
        else {
            return false;
        };
        let Some(old_root) = self.popups.get(&id).and_then(|entry| entry.root.clone()) else {
            return false;
        };
        let visibility = self
            .popups
            .get(&id)
            .and_then(|entry| entry.visibility.clone());

        // Reuse the current revealer so one id still has one stack row
        let new_root = self.build_popup_root(notification);
        let rebuilt_visible_row = old_root.is_visible() || revealer.reveals_child();
        if old_root.is_visible() {
            new_root.set_visible(true);
        }
        if old_root.has_css_class("unixnotis-popup-visible") {
            new_root.add_css_class("unixnotis-popup-visible");
        }
        if self.config.theme.notification_corners.is_active() {
            if let Some(plate) = revealer.child().and_downcast::<CutCorner>() {
                // Preserve the reveal animation while swapping only the clipped card contents
                plate.set_child(Some(&new_root));
                plate.set_corners(self.config.theme.notification_corners);
            } else {
                // Enabling experimental cuts replaces the ordinary card wrapper on rebuild
                let plate = CutCorner::new(&new_root, self.config.theme.notification_corners);
                revealer.set_child(Some(&plate));
            }
        } else {
            // Disabling experimental cuts restores the native rounded card shape
            revealer.set_child(Some(&new_root));
        }

        if let Some(entry) = self.popups.get_mut(&id) {
            entry.root = Some(new_root);
        }
        try_send_command(
            &self.command_tx,
            UiCommand::Materialized(notification.key()),
        );
        if let Some(visibility) = visibility {
            // Replacements reuse one revealer but never reuse its generation identity
            visibility.bind_generation(notification.key());
            visibility.report_if_visible(&revealer, &self.popup_window, &self.command_tx);
        }
        rebuilt_visible_row
    }

    pub(super) fn materialize_popup(&mut self, id: u32) {
        // Visible rows get rebuilt from the stored payload only when they are actually needed
        let notification = match self.popups.get(&id) {
            Some(entry) if !entry.is_materialized() => entry.notification.clone(),
            _ => return,
        };
        let built = self.build_popup_entry(&notification);
        let Some(entry) = self.popups.get_mut(&id) else {
            return;
        };
        // Swap in the fresh GTK nodes while keeping the cached payload untouched
        entry.revealer = built.revealer;
        entry.root = built.root;
        entry.visibility = built.visibility;
    }

    pub(super) fn dematerialize_popup(&mut self, id: u32) {
        // Hidden rows keep only plain Rust data so backlog size does not scale GTK memory
        let Some(entry) = self.popups.get_mut(&id) else {
            return;
        };
        let Some(root) = entry.root.take() else {
            entry.revealer = None;
            entry.visibility = None;
            return;
        };
        let Some(revealer) = entry.revealer.take() else {
            entry.visibility = None;
            return;
        };
        entry.visibility = None;
        // Hidden overflow rows should not retain GTK trees or CSS state
        root.remove_css_class("unixnotis-popup-visible");
        root.set_visible(false);
        revealer.set_reveal_child(false);
        if revealer.parent().is_some() {
            self.popup_stack.remove(&revealer);
        }
    }
}

#[cfg(test)]
#[path = "tests/mutation.rs"]
mod tests;
