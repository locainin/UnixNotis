use std::sync::Arc;
use std::time::Instant;

use unixnotis_core::{CloseReason, Notification};

use crate::store::{DismissOutcome, NotificationStore};

impl NotificationStore {
    pub fn close(&mut self, id: u32, reason: CloseReason) -> Option<Arc<Notification>> {
        // Active removal and expiration cleanup always happen together
        let removed = self.active.shift_remove(&id);
        self.expirations.remove(&id);
        if let Some(notification) = removed.clone() {
            // Closed rows and panel rows should follow the same archive rule
            self.push_history(notification, reason);
        }
        removed
    }

    pub fn dismiss_from_panel(&mut self, id: u32) -> DismissOutcome {
        // Panel dismissal can target active, history, or both
        let removed_active = self.active.shift_remove(&id).is_some();
        if removed_active {
            self.expirations.remove(&id);
        }

        let removed_history = self.history.remove(&id).is_some();

        DismissOutcome {
            removed_active,
            removed_history,
        }
    }

    pub fn dismiss_active_if_current(&mut self, id: u32, expected: &Arc<Notification>) -> bool {
        // A replacement can reuse the numeric ID but never the same Arc allocation
        let is_current = self
            .active
            .get(&id)
            .is_some_and(|active| Arc::ptr_eq(active, expected));
        if !is_current {
            // Keep a replacement that arrived while an earlier action was in flight
            return false;
        }

        self.active.shift_remove(&id);
        self.expirations.remove(&id);
        true
    }

    pub fn dismiss_replied_generation(
        &mut self,
        id: u32,
        expected: &Arc<Notification>,
    ) -> DismissOutcome {
        let removed_active = self.dismiss_active_if_current(id, expected);
        let removed_history = if removed_active {
            // Active cleanup already removed the exact generation
            false
        } else if self.active.contains_key(&id) {
            // Any remaining active entry is a replacement with the same numeric id
            false
        } else {
            // A close may archive the replied generation before reply cleanup resumes
            self.history.remove_if_source(id, expected).is_some()
        };
        DismissOutcome {
            removed_active,
            removed_history,
        }
    }

    pub fn drain_active_ids(&mut self) -> Vec<u32> {
        // Drain in one pass so callers do not need repeated lookups
        let ids = self.active.keys().rev().copied().collect();
        self.active.clear();
        self.expirations.clear();
        ids
    }

    pub fn set_expiration(&mut self, id: u32, deadline: Option<Instant>) {
        // None removes a stale timer for resident or already-dismissed notifications
        match deadline {
            Some(deadline) => {
                self.expirations.insert(id, deadline);
            }
            None => {
                self.expirations.remove(&id);
            }
        }
    }

    pub fn expiration_for(&self, id: u32) -> Option<Instant> {
        self.expirations.get(&id).copied()
    }
}
