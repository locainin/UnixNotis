//! Popup list management and visibility updates.
//!
//! Focuses on maintaining popup ordering and visibility rules.

use std::collections::{HashMap, HashSet, VecDeque};

use gtk::prelude::*;
use tracing::{debug, warn};
use unixnotis_core::{popup_allowed_by_state, ControlState, NotificationView};

use super::ui_window::{popup_stack_has_active_transitions, refresh_popup_input_region};
use super::{PopupEntry, UiState};

struct ReconcilePlan {
    // Local rows missing from the daemon snapshot
    stale_ids: Vec<u32>,
    // Rows that must be inserted or updated to match daemon truth
    updates: Vec<NotificationView>,
    // Final order copied from the daemon seed
    desired_order: VecDeque<u32>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct VisiblePopupUpdate {
    // True when stack order, materialization, or reveal state changed
    stack_changed: bool,
}

fn visible_popup_restack_ids(previous_visible: &[u32], desired_visible: &[u32]) -> HashSet<u32> {
    // Work on a local order copy so move decisions stay deterministic and testable
    let mut working = previous_visible.to_vec();
    let mut positions = previous_visible
        .iter()
        .copied()
        .enumerate()
        .map(|(index, id)| (id, index))
        .collect::<HashMap<u32, usize>>();
    let mut moved = HashSet::new();

    for (target_index, id) in desired_visible.iter().copied().enumerate() {
        let Some(current_index) = positions.get(&id).copied() else {
            // New ids need one attach at their final slot
            working.insert(target_index, id);
            for (index, current_id) in working.iter().copied().enumerate().skip(target_index) {
                positions.insert(current_id, index);
            }
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
        let start = target_index.min(current_index);
        let end = target_index.max(current_index);
        for (index, current_id) in working
            .iter()
            .copied()
            .enumerate()
            .skip(start)
            .take(end - start + 1)
        {
            positions.insert(current_id, index);
        }
        moved.insert(id);
    }

    moved
}

impl UiState {
    pub(super) fn reconcile_seed(&mut self, active: Vec<NotificationView>) {
        // Seed is a full snapshot, so desired popups come only from this list
        let desired = desired_seed_popups(active, &self.control_state);
        // Compare only the portable notification payload so seed logic stays deterministic
        let local = self
            .popups
            .iter()
            .map(|(id, entry)| (*id, entry.notification.clone()))
            .collect();
        let plan = build_reconcile_plan(&local, &self.popup_order, &desired);

        // Remove old ids first so inserts and updates work on the final set
        for id in plan.stale_ids {
            self.remove_popup_internal(id, false);
        }

        // Walk oldest to newest so front insertion lands in daemon order
        let mut force_region_refresh = false;
        for notification in plan.updates.iter().rev() {
            force_region_refresh |= self.update_popup_internal(notification.clone(), true, false);
        }

        // Seed order wins even if local insert timing was different before reconnect
        self.popup_order = plan.desired_order;
        self.update_popup_visibility(force_region_refresh);
        debug!(total = self.popup_order.len(), "popup seed reconciled");
    }

    pub(super) fn retain_allowed_popups(&mut self) {
        // State changes only remove popups that are no longer allowed
        let remove_ids: Vec<u32> = self
            .popups
            .iter()
            .filter_map(|(id, entry)| {
                (!popup_allowed_by_state(entry.notification.urgency, &self.control_state))
                    .then_some(*id)
            })
            .collect();
        let removed_any = !remove_ids.is_empty();
        for id in remove_ids {
            self.remove_popup_internal(id, false);
        }
        if removed_any {
            self.update_popup_visibility(false);
        }
    }

    pub(super) fn add_popup(&mut self, notification: NotificationView) {
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
            self.remove_popup_internal(id, refresh_visibility);
            return false;
        }

        if !self.popups.contains_key(&id) {
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
            entry.notification = notification;
        }

        if refresh_visibility {
            self.update_popup_visibility(rebuilt_visible_row);
        }
        debug!(id, "popup updated");
        rebuilt_visible_row
    }

    pub(super) fn remove_popup(&mut self, id: u32) {
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
                    let has_active_transitions = popup_stack_has_active_transitions(&stack);
                    refresh_popup_input_region(
                        &popup_window,
                        &stack,
                        &popup_input_region,
                        has_active_transitions,
                    );
                });
            }
        }
        self.popup_order.retain(|item| *item != id);
        if refresh_visibility {
            self.update_popup_visibility(false);
        }
        debug!(id, total = self.popup_order.len(), "popup removed");
    }

    pub(super) fn update_popup_visibility(&mut self, force_region_refresh: bool) {
        // Visibility contract is driven strictly by configured max_visible count
        let max_visible = self.config.popups.max_visible;

        // Max-visible of zero disables popups entirely.
        if max_visible == 0 {
            let update = self.apply_visible_popups(Vec::new());
            self.popup_window.set_visible(false);
            // Keep input region empty when popups are disabled
            if force_region_refresh || update.stack_changed {
                refresh_popup_input_region(
                    &self.popup_window,
                    &self.popup_stack,
                    &self.popup_input_region,
                    false,
                );
            }
            debug!("popups disabled by max_visible = 0");
            return;
        }

        // Hide the top-level window when there are no active popups.
        if self.popup_order.is_empty() {
            self.popup_window.set_visible(false);
        } else {
            self.popup_window.set_visible(true);
        }

        // Only the leading visible slice should pay GTK visibility churn on every update
        let desired_visible = self
            .popup_order
            .iter()
            .take(visible_popup_target(self.popup_order.len(), max_visible))
            .copied()
            .collect::<Vec<u32>>();
        let update = self.apply_visible_popups(desired_visible);
        // Tick while transitions run so interactive area tracks animation frames
        let has_active_transitions = popup_stack_has_active_transitions(&self.popup_stack);
        if force_region_refresh || update.stack_changed || has_active_transitions {
            refresh_popup_input_region(
                &self.popup_window,
                &self.popup_stack,
                &self.popup_input_region,
                has_active_transitions,
            );
        }
        debug!(
            visible = self.visible_popups.len(),
            total = self.popup_order.len(),
            "popup visibility updated"
        );
    }

    pub(super) fn refresh_after_config_reload(&mut self) {
        let resized_roots = self
            .popups
            .values()
            .filter(|entry| entry.root.is_some())
            .count();
        // Prefer the live width when GTK has already measured the stack
        let popup_width = self
            .popup_stack
            .width()
            .max(self.popup_stack.width_request())
            .max(1);
        for entry in self.popups.values() {
            let Some(root) = entry.root.as_ref() else {
                continue;
            };
            root.set_size_request(popup_width, -1);
        }
        // Re-run visibility so max_visible changes take effect right away
        self.update_popup_visibility(true);
        debug!(
            resized_roots,
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
        for id in &previous_visible {
            if desired_visible_set.contains(id) {
                continue;
            }
            self.dematerialize_popup(*id);
            update.stack_changed = true;
        }

        // Attach or move only the rows that actually changed order
        let mut previous_revealer: Option<gtk::Revealer> = None;
        for id in &desired_visible {
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
        update
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

fn visible_popup_target(total_popups: usize, max_visible: usize) -> usize {
    // Visible slice can never exceed the number of known popups
    total_popups.min(max_visible)
}

fn build_reconcile_plan(
    local: &HashMap<u32, NotificationView>,
    local_order: &VecDeque<u32>,
    desired: &[NotificationView],
) -> ReconcilePlan {
    let desired_order = desired
        .iter()
        .map(|notification| notification.id)
        .collect::<VecDeque<u32>>();
    let desired_ids = desired
        .iter()
        .map(|notification| notification.id)
        .collect::<HashSet<u32>>();

    // Local rows that the daemon no longer lists must be removed
    let stale_ids = local_order
        .iter()
        .copied()
        .filter(|id| !desired_ids.contains(id))
        .collect::<Vec<u32>>();
    // Seed order is restored by popup_order, so only payload changes need a rebuild
    let updates = desired
        .iter()
        .filter(|notification| match local.get(&notification.id) {
            // Identical rows can stay as they are while visibility fixes order later
            Some(existing) => existing != *notification,
            // Missing rows must be inserted from seed
            None => true,
        })
        .cloned()
        .collect::<Vec<NotificationView>>();

    ReconcilePlan {
        stale_ids,
        updates,
        desired_order,
    }
}

fn desired_seed_popups(
    active: Vec<NotificationView>,
    state: &ControlState,
) -> Vec<NotificationView> {
    // Seed filtering uses the same gate as runtime state changes
    // This keeps reconnect snapshots and live signals on the same visibility rules
    active
        .into_iter()
        .filter(|notification| popup_allowed_by_state(notification.urgency, state))
        .collect()
}

#[cfg(test)]
#[path = "ui_popups_tests.rs"]
mod tests;
