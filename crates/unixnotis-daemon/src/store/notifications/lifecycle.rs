use std::sync::Arc;
use std::time::Instant;

use unixnotis_core::{CloseReason, Notification, NotificationKey};

use crate::store::{DismissOutcome, ExpirationTicket, NotificationStore};

impl NotificationStore {
    pub fn close(&mut self, id: u32, reason: CloseReason) -> Option<Arc<Notification>> {
        // Active removal and expiration cleanup always happen together
        let removed = self.active.shift_remove(&id);
        self.expirations.remove(&id);
        if let Some(notification) = removed.clone() {
            // Closed rows and panel rows should follow the same archive rule
            self.push_history(notification, reason);
        }
        self.prune_popup_decisions();
        removed
    }

    pub fn dismiss_generation(&mut self, key: NotificationKey) -> DismissOutcome {
        // Validate the generation before mutating either active or retained history state
        let active_matches = self
            .active
            .get(&key.id)
            .is_some_and(|notification| notification.generation == key.generation);
        let removed_active = if active_matches {
            let removed = self.active.shift_remove(&key.id);
            self.expirations.remove(&key.id);
            removed.map(|notification| notification.key())
        } else {
            None
        };
        let removed_history = if removed_active.is_some() {
            None
        } else {
            self.history
                .remove_generation(key)
                .map(|notification| notification.key())
        };

        let outcome = DismissOutcome {
            removed_active,
            removed_history,
        };
        self.prune_popup_decisions();
        outcome
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
        let removed_active = self
            .dismiss_active_if_current(id, expected)
            .then(|| expected.key());
        let removed_history = if removed_active.is_some() {
            // Active cleanup already removed the exact generation
            None
        } else if self.active.contains_key(&id) {
            // Any remaining active entry is a replacement with the same numeric id
            None
        } else {
            // A close may archive the replied generation before reply cleanup resumes
            self.history
                .remove_if_source(id, expected)
                .map(|notification| notification.key())
        };
        let outcome = DismissOutcome {
            removed_active,
            removed_history,
        };
        self.prune_popup_decisions();
        outcome
    }

    pub fn drain_active_keys(&mut self) -> Vec<NotificationKey> {
        // Drain in one pass so callers do not need repeated lookups
        let keys = self
            .active
            .values()
            .rev()
            .map(|notification| notification.key())
            .collect();
        self.active.clear();
        self.expirations.clear();
        self.prune_popup_decisions();
        keys
    }

    pub fn set_expiration(
        &mut self,
        notification: &Arc<Notification>,
        deadline: Option<Instant>,
    ) -> Option<ExpirationTicket> {
        // None removes a stale timer for resident or already-dismissed notifications
        if let Some(deadline) = deadline {
            let ticket = ExpirationTicket {
                id: notification.id,
                generation: notification.generation,
                deadline,
            };
            self.expirations.insert(notification.id, ticket);
            Some(ticket)
        } else {
            self.expirations.remove(&notification.id);
            None
        }
    }

    pub fn expire_if_current(&mut self, ticket: ExpirationTicket) -> Option<Arc<Notification>> {
        // Both identities must match inside this one store-lock critical section
        let current = self.active.get(&ticket.id)?;
        if current.generation != ticket.generation {
            return None;
        }
        if self.expirations.get(&ticket.id) != Some(&ticket) {
            return None;
        }

        // Removal, timer cleanup, and history insertion commit atomically
        let removed = self.active.shift_remove(&ticket.id)?;
        self.expirations.remove(&ticket.id);
        self.push_history(removed.clone(), CloseReason::Expired);
        self.prune_popup_decisions();
        Some(removed)
    }
}
