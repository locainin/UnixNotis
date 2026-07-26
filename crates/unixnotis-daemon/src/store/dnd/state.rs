use crate::store::{DndWrite, NotificationStore};

impl NotificationStore {
    pub const fn dnd_enabled(&self) -> bool {
        self.dnd_enabled
    }

    pub const fn dnd_expires_at(&self) -> Option<i64> {
        self.dnd_expires_at
    }

    pub fn set_dnd(&mut self, enabled: bool) -> DndWrite {
        // A plain set always means indefinite when enabled
        self.write_dnd(enabled, None)
    }

    pub fn set_dnd_until(&mut self, expires_at: i64) -> DndWrite {
        // Validation happens at the control boundary before this state mutation
        self.write_dnd(true, Some(expires_at))
    }

    pub fn toggle_dnd(&mut self) -> DndWrite {
        // Toggle and write happen under one lock at the call site
        self.write_dnd(!self.dnd_enabled, None)
    }

    pub fn expire_dnd_if_current(&mut self, expires_at: i64, now: i64) -> DndWrite {
        if !self.dnd_enabled || self.dnd_expires_at != Some(expires_at) || expires_at > now {
            // A replaced or not-yet-due schedule cannot alter current state
            return self.unchanged_dnd_write();
        }
        self.write_dnd(false, None)
    }

    pub(crate) fn rollback_dnd_write_if_current(&mut self, write: &DndWrite) -> bool {
        // No-op writes do not need rollback
        if !write.changed {
            return false;
        }
        // Guarded rollback avoids clobbering newer successful writes
        if self.dnd_revision != write.revision
            || self.dnd_enabled != write.current
            || self.dnd_expires_at != write.current_expires_at
        {
            return false;
        }
        self.dnd_enabled = write.previous;
        self.dnd_expires_at = write.previous_expires_at;
        // Rollback is also a state transition
        self.dnd_revision = self.dnd_revision.saturating_add(1);
        true
    }

    fn write_dnd(&mut self, enabled: bool, expires_at: Option<i64>) -> DndWrite {
        // Disabled DND cannot retain a deadline
        let expires_at = enabled.then_some(expires_at).flatten();
        let previous = self.dnd_enabled;
        let previous_expires_at = self.dnd_expires_at;
        if previous == enabled && previous_expires_at == expires_at {
            // Returning unchanged avoids unnecessary disk writes and state signals
            return self.unchanged_dnd_write();
        }
        self.dnd_enabled = enabled;
        self.dnd_expires_at = expires_at;
        self.dnd_revision = self.dnd_revision.saturating_add(1);
        // Persist outside the store lock so notification flow stays responsive
        DndWrite {
            changed: true,
            previous,
            previous_expires_at,
            current: enabled,
            current_expires_at: expires_at,
            revision: self.dnd_revision,
            persist: self.dnd_state_store.clone(),
        }
    }

    const fn unchanged_dnd_write(&self) -> DndWrite {
        DndWrite {
            changed: false,
            previous: self.dnd_enabled,
            previous_expires_at: self.dnd_expires_at,
            current: self.dnd_enabled,
            current_expires_at: self.dnd_expires_at,
            revision: self.dnd_revision,
            persist: None,
        }
    }
}
