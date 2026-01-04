//! Notification store with ordering and history management.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use indexmap::IndexMap;
use unixnotis_core::{Config, Notification, NotificationView, Urgency};

/// Mutable notification state owned by the daemon.
pub struct NotificationStore {
    config: Config,
    next_id: u32,
    active: IndexMap<u32, Arc<Notification>>,
    history: IndexMap<u32, Arc<Notification>>,
    expirations: HashMap<u32, Instant>,
    dnd_enabled: bool,
}

pub struct InsertOutcome {
    pub notification: Arc<Notification>,
    pub replaced: bool,
    pub show_popup: bool,
    pub evicted: Vec<u32>,
}

pub struct DismissOutcome {
    pub removed_active: bool,
    pub removed_history: bool,
}

impl DismissOutcome {
    pub fn removed_any(&self) -> bool {
        self.removed_active || self.removed_history
    }
}

impl NotificationStore {
    pub fn new(config: Config) -> Self {
        Self {
            next_id: 1,
            dnd_enabled: config.general.dnd_default,
            config,
            active: IndexMap::new(),
            history: IndexMap::new(),
            expirations: HashMap::new(),
        }
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn dnd_enabled(&self) -> bool {
        self.dnd_enabled
    }

    pub fn set_dnd(&mut self, enabled: bool) {
        self.dnd_enabled = enabled;
    }

    pub fn list_active(&self) -> Vec<NotificationView> {
        self.active
            .values()
            .rev()
            .map(|notification| notification.to_list_view())
            .collect()
    }

    pub fn list_history(&self) -> Vec<NotificationView> {
        self.history
            .values()
            .rev()
            .map(|notification| notification.to_list_view())
            .collect()
    }

    pub fn history_len(&self) -> usize {
        self.history.len()
    }

    pub fn insert(&mut self, mut notification: Notification, replaces_id: u32) -> InsertOutcome {
        // Preserve protocol semantics: replaces_id only applies when it matches an existing item.
        let has_replaces_id = replaces_id != 0;
        // Replacement is only true when the referenced notification is present.
        let replaced = has_replaces_id
            && (self.active.contains_key(&replaces_id) || self.history.contains_key(&replaces_id));
        let assigned_id = if replaced {
            replaces_id
        } else {
            self.next_id()
        };
        notification.id = assigned_id;

        // Remove any stale entries for this ID before inserting the replacement.
        self.active.shift_remove(&assigned_id);
        self.history.shift_remove(&assigned_id);
        self.expirations.remove(&assigned_id);

        let notification = Arc::new(notification);
        self.active.insert(assigned_id, notification.clone());
        let evicted = self.enforce_active_limit();

        InsertOutcome {
            show_popup: self.should_show_popup(&notification),
            notification,
            replaced,
            evicted,
        }
    }

    pub fn close(&mut self, id: u32) -> Option<Arc<Notification>> {
        let removed = self.active.shift_remove(&id);
        self.expirations.remove(&id);
        if let Some(notification) = removed.clone() {
            // History entries are appended only when the notification is explicitly closed.
            self.push_history(notification.clone());
        }
        removed
    }

    pub fn clear_history(&mut self) {
        self.history.clear();
    }

    pub fn dismiss_from_panel(&mut self, id: u32) -> DismissOutcome {
        let removed_active = self.active.shift_remove(&id).is_some();
        if removed_active {
            self.expirations.remove(&id);
        }

        let removed_history = self.history.shift_remove(&id).is_some();

        DismissOutcome {
            removed_active,
            removed_history,
        }
    }

    pub fn drain_active_ids(&mut self) -> Vec<u32> {
        // Drain active notifications in one pass to avoid repeated scans.
        let ids = self.active.keys().rev().copied().collect();
        self.active.clear();
        self.expirations.clear();
        ids
    }

    pub fn set_expiration(&mut self, id: u32, deadline: Option<Instant>) {
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

    fn next_id(&mut self) -> u32 {
        let start = self.next_id.max(1);
        let mut candidate = start;
        loop {
            if !self.active.contains_key(&candidate) && !self.history.contains_key(&candidate) {
                self.next_id = candidate.wrapping_add(1);
                if self.next_id == 0 {
                    self.next_id = 1;
                }
                return candidate;
            }
            candidate = candidate.wrapping_add(1);
            if candidate == 0 {
                candidate = 1;
            }
            if candidate == start {
                return candidate;
            }
        }
    }

    fn enforce_active_limit(&mut self) -> Vec<u32> {
        let max_active = self.config.history.max_active;
        if max_active == 0 {
            return Vec::new();
        }
        let mut evicted = Vec::new();
        while self.active.len() > max_active {
            if let Some((id, notification)) = self.active.shift_remove_index(0) {
                self.expirations.remove(&id);
                self.push_history(notification);
                evicted.push(id);
            } else {
                break;
            }
        }
        evicted
    }

    fn push_history(&mut self, notification: Arc<Notification>) {
        if notification.is_transient && !self.config.history.transient_to_history {
            return;
        }
        let id = notification.id;
        self.history.shift_remove(&id);
        let stored = Arc::new(notification.to_history());
        self.history.insert(id, stored);
        while self.history.len() > self.config.history.max_entries {
            let _ = self.history.shift_remove_index(0);
        }
    }

    fn should_show_popup(&self, notification: &Notification) -> bool {
        if self.dnd_enabled {
            return notification.urgency == Urgency::Critical;
        }
        true
    }
}
