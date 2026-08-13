use std::borrow::Borrow;
use std::collections::{HashMap, HashSet, VecDeque};

use tracing::debug;
use unixnotis_core::{popup_allowed_by_state, ControlState, NotificationView, PopupDeliveryStage};

use super::super::UiState;
use super::mutation::ReconcilePlan;

impl UiState {
    pub(in super::super) fn reconcile_seed(&mut self, active: Vec<NotificationView>) {
        // Refresh source indexes before deciding which materialized rows need rebuilding
        self.refresh_icon_sources_if_needed();
        // Seed is a full snapshot, so desired popups come only from this list
        let desired = desired_seed_popups(active, &self.control_state)
            .into_iter()
            .filter(|notification| !self.hidden_popups.contains(&notification.key()))
            .collect::<Vec<_>>();
        // Compare only the portable notification payload so seed logic stays deterministic
        let local = self
            .popups
            .iter()
            .map(|(id, entry)| (*id, &entry.notification))
            .collect();
        let refresh_icons = desired.iter().any(|notification| {
            self.popups.get(&notification.id).is_some_and(|entry| {
                entry.is_materialized()
                    && entry.icon_source_generation != self.icon_source_generation
            })
        });
        let plan = build_reconcile_plan_with_icon_refresh(
            &local,
            &self.popup_order,
            &desired,
            refresh_icons,
        );

        // Queued rows have no GTK tree to rebuild, so advance them to the current source generation
        for notification in &desired {
            if let Some(entry) = self.popups.get_mut(&notification.id) {
                if !entry.is_materialized() {
                    entry.icon_source_generation = self.icon_source_generation;
                }
            }
        }

        // Remove old ids first so inserts and updates work on the final set
        for id in plan.stale_ids {
            self.remove_popup_internal(id, false);
        }

        // Walk oldest to newest so front insertion lands in daemon order
        let mut force_region_refresh = false;
        for notification in plan.updates.into_iter().rev() {
            force_region_refresh |= self.update_popup_internal(notification, true, false);
        }

        // Seed order wins even if local insert timing was different before reconnect
        self.popup_order = plan.desired_order;
        self.update_popup_visibility(force_region_refresh);
        debug!(total = self.popup_order.len(), "popup seed reconciled");
    }

    pub(in super::super) fn retain_allowed_popups(&mut self) {
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
}

fn build_reconcile_plan_with_icon_refresh<T>(
    local: &HashMap<u32, T>,
    local_order: &VecDeque<u32>,
    desired: &[NotificationView],
    refresh_icons: bool,
) -> ReconcilePlan
where
    T: Borrow<NotificationView>,
{
    // Desired order comes straight from the daemon snapshot
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
            Some(existing) => refresh_icons || existing.borrow() != *notification,
            // Missing rows must be inserted from seed
            None => true,
        })
        .cloned()
        .collect::<Vec<NotificationView>>();

    ReconcilePlan {
        // Plan keeps remove, update, and order work in one place
        stale_ids,
        updates,
        desired_order,
    }
}

pub(super) fn desired_seed_popups(
    active: Vec<NotificationView>,
    state: &ControlState,
) -> Vec<NotificationView> {
    // Seed filtering uses the same gate as runtime state changes
    // This keeps reconnect snapshots and live signals on the same visibility rules
    active
        .into_iter()
        .filter(|notification| {
            popup_allowed_by_state(notification.urgency, state)
                && notification.popup_decision.delivery_stage.rank()
                    < PopupDeliveryStage::Visible.rank()
        })
        .collect()
}

#[cfg(test)]
#[path = "tests/reconcile.rs"]
mod tests;
