use super::{DndWrite, NotificationStore};

impl NotificationStore {
    pub fn dnd_enabled(&self) -> bool {
        self.dnd_enabled
    }

    pub fn set_dnd(&mut self, enabled: bool) -> DndWrite {
        // Shared mutation path keeps set and toggle behavior aligned
        self.write_dnd(enabled)
    }

    pub fn toggle_dnd(&mut self) -> DndWrite {
        // Toggle and write happen under one lock at the call site
        self.write_dnd(!self.dnd_enabled)
    }

    pub(crate) fn rollback_dnd_write_if_current(&mut self, write: &DndWrite) -> bool {
        // No-op writes do not need rollback
        if !write.changed {
            return false;
        }
        // Guarded rollback avoids clobbering newer successful writes
        if self.dnd_revision != write.revision || self.dnd_enabled != write.current {
            return false;
        }
        self.dnd_enabled = write.previous;
        // Rollback is also a state transition
        self.dnd_revision = self.dnd_revision.saturating_add(1);
        true
    }

    fn write_dnd(&mut self, enabled: bool) -> DndWrite {
        let previous = self.dnd_enabled;
        if previous == enabled {
            // Returning unchanged avoids unnecessary disk writes and state signals
            return DndWrite {
                changed: false,
                previous,
                current: previous,
                revision: self.dnd_revision,
                persist: None,
            };
        }
        self.dnd_enabled = enabled;
        self.dnd_revision = self.dnd_revision.saturating_add(1);
        // Persist outside the store lock so notification flow stays responsive
        DndWrite {
            changed: true,
            previous,
            current: enabled,
            revision: self.dnd_revision,
            persist: self.dnd_state_store.clone(),
        }
    }
}
