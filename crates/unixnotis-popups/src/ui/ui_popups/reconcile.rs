use std::borrow::Borrow;
use std::collections::{HashMap, HashSet, VecDeque};

use tracing::debug;
use unixnotis_core::{popup_allowed_by_state, ControlState, NotificationView};

use super::{ReconcilePlan, UiState};

impl UiState {
    pub(in super::super) fn reconcile_seed(&mut self, active: Vec<NotificationView>) {
        // Seed is a full snapshot, so desired popups come only from this list
        let desired = desired_seed_popups(active, &self.control_state);
        // Compare only the portable notification payload so seed logic stays deterministic
        let local = self
            .popups
            .iter()
            .map(|(id, entry)| (*id, &entry.notification))
            .collect();
        let plan = build_reconcile_plan(&local, &self.popup_order, &desired);

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

pub(super) fn build_reconcile_plan<T>(
    local: &HashMap<u32, T>,
    local_order: &VecDeque<u32>,
    desired: &[NotificationView],
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
            Some(existing) => existing.borrow() != *notification,
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
        .filter(|notification| popup_allowed_by_state(notification.urgency, state))
        .collect()
}
