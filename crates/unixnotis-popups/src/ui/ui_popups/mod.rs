//! Popup list management and visibility updates
//!
//! Keeps popup reconcile, mutation, and visibility logic in focused files

mod reconcile;
mod visibility;

use gtk::prelude::*;
use tracing::debug;
use unixnotis_core::NotificationView;

use super::ui_window::refresh_popup_input_region;
use super::{PopupEntry, UiState};

pub(super) struct ReconcilePlan {
    // Local rows missing from the daemon snapshot
    stale_ids: Vec<u32>,
    // Rows that must be inserted or updated to match daemon truth
    updates: Vec<NotificationView>,
    // Final order copied from the daemon seed
    desired_order: std::collections::VecDeque<u32>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct VisiblePopupUpdate {
    // True when stack order, materialization, or reveal state changed
    stack_changed: bool,
}

impl UiState {
    pub(super) fn add_popup(&mut self, notification: NotificationView) {
        // Runtime insert path keeps one place for add semantics
        self.add_popup_internal(notification, true);
    }

    fn add_popup_internal(&mut self, notification: NotificationView, refresh_visibility: bool) {
        let id = notification.id;
        // Duplicate ids point at an upstream state bug
        if self.popups.contains_key(&id) {
            debug!(id, "popup insert skipped because id already exists");
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

    pub(super) fn update_popup(&mut self, notification: NotificationView, show_popup: bool) {
        // Update path can also hide a popup when policy says not to show it
        self.update_popup_internal(notification, show_popup, true);
    }

    fn update_popup_internal(
        &mut self,
        notification: NotificationView,
        show_popup: bool,
        refresh_visibility: bool,
    ) -> bool {
        let id = notification.id;
        if !show_popup {
            // Hidden updates act like a close for this popup id
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

    pub(super) fn remove_popup(&mut self, id: u32) {
        // Runtime close path keeps one place for remove semantics
        self.remove_popup_internal(id, true);
    }

    fn remove_popup_internal(&mut self, id: u32, refresh_visibility: bool) {
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

        // Reuse the current revealer so one id still has one stack row
        let new_root = self.build_popup_root(notification);
        let rebuilt_visible_row = old_root.is_visible() || revealer.reveals_child();
        if old_root.is_visible() {
            new_root.set_visible(true);
        }
        if old_root.has_css_class("unixnotis-popup-visible") {
            new_root.add_css_class("unixnotis-popup-visible");
        }
        revealer.set_child(Some(&new_root));

        if let Some(entry) = self.popups.get_mut(&id) {
            entry.root = Some(new_root);
        }
        rebuilt_visible_row
    }

    fn materialize_popup(&mut self, id: u32) {
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
    }

    fn dematerialize_popup(&mut self, id: u32) {
        // Hidden rows keep only plain Rust data so backlog size does not scale GTK memory
        let Some(entry) = self.popups.get_mut(&id) else {
            return;
        };
        let Some(root) = entry.root.take() else {
            entry.revealer = None;
            return;
        };
        let Some(revealer) = entry.revealer.take() else {
            return;
        };
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
mod tests;
