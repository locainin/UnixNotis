//! Notification store with ordering, history, and suppression policies
//!
//! store.rs stays as the main entry point and wires focused modules under store/

// Focused modules keep policy and lifecycle logic isolated and easier to test
#[path = "store/store_history.rs"]
mod store_history;
#[path = "store/store_identity.rs"]
mod store_identity;
#[path = "store/store_inhibit.rs"]
mod store_inhibit;
#[path = "store/store_inhibitor_api.rs"]
mod store_inhibitor_api;
#[path = "store/store_lifecycle.rs"]
mod store_lifecycle;
#[path = "store/store_rules.rs"]
mod store_rules;
#[path = "store/store_state.rs"]
mod store_state;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use indexmap::IndexMap;
use tracing::{debug, warn};
use unixnotis_core::{Config, Notification, NotificationView};

// Internal store primitives used by the main NotificationStore type
use store_history::HistoryStore;
use store_inhibit::Inhibitor;
use store_state::{DndStateStore, DND_STATE_VERSION};

#[cfg(test)]
use std::path::PathBuf;

#[cfg(test)]
use store_rules::contains_ci;

/// Mutable notification state owned by the daemon
pub struct NotificationStore {
    // Immutable runtime config snapshot
    config: Config,
    // Next candidate id for allocation
    next_id: u32,
    // Active notifications in insertion order
    active: IndexMap<u32, Arc<Notification>>,
    // Archived notifications with bounded retention
    history: HistoryStore,
    // Optional expiration deadline per active id
    expirations: HashMap<u32, Instant>,
    // Effective DND switch after loading persisted state
    dnd_enabled: bool,
    // Monotonic in-memory revision for DND writes
    dnd_revision: u64,
    // Optional persistence layer for DND; absent store keeps behavior in-memory
    dnd_state_store: Option<DndStateStore>,
    // Token counter for inhibitors; never reused in a process
    next_inhibitor_id: u64,
    // Active inhibitors keyed by token for quick lookup/removal
    inhibitors: HashMap<u64, Inhibitor>,
    // Cached flags avoid rescanning inhibitors on every notification
    inhibited: bool,
    inhibitor_count: u32,
}

pub struct InsertOutcome {
    // Stored notification instance returned to callers
    pub notification: Arc<Notification>,
    // True when insertion replaced an existing id
    pub replaced: bool,
    // Whether popup rendering is allowed for this payload
    pub show_popup: bool,
    // Whether sound playback is allowed for this payload
    pub allow_sound: bool,
    // Active ids evicted because max_active was exceeded
    pub evicted: Vec<u32>,
    // True when payload was intentionally dropped by inhibit mode
    pub dropped: bool,
}

pub(crate) struct DndWrite {
    // True when the in-memory DND value changed
    pub(crate) changed: bool,
    // Value seen before this write
    pub(crate) previous: bool,
    // Value written by this operation
    pub(crate) current: bool,
    // Monotonic revision captured for guarded rollback
    pub(crate) revision: u64,
    // Persistence backend used outside the store lock
    pub(crate) persist: Option<DndStateStore>,
}

pub struct DismissOutcome {
    // True when an active entry was removed
    pub removed_active: bool,
    // True when a history entry was removed
    pub removed_history: bool,
}

impl DismissOutcome {
    pub fn removed_any(&self) -> bool {
        // Convenience helper for callers that only need yes/no
        self.removed_active || self.removed_history
    }
}

impl NotificationStore {
    pub fn new(config: Config) -> Self {
        // Default constructor attempts to bind persistence to XDG state dir
        let dnd_state_store = DndStateStore::new();
        Self::new_with_state_store(config, dnd_state_store)
    }

    #[cfg(test)]
    pub(crate) fn new_with_state_dir(config: Config, state_dir: PathBuf) -> Self {
        // Test helper with explicit state path and no env mutations
        let dnd_state_store = Some(DndStateStore::from_state_dir(state_dir));
        Self::new_with_state_store(config, dnd_state_store)
    }

    fn new_with_state_store(config: Config, dnd_state_store: Option<DndStateStore>) -> Self {
        // Config default is used unless a valid persisted value overrides it
        let mut dnd_enabled = config.general.dnd_default;
        if let Some(store) = dnd_state_store.as_ref() {
            match store.load() {
                Ok(Some(state)) if state.version == DND_STATE_VERSION => {
                    // Versioned state prevents accidental decode of incompatible formats
                    dnd_enabled = state.dnd_enabled;
                    debug!(dnd_enabled, "loaded persisted do-not-disturb state");
                }
                Ok(Some(state)) => {
                    // Unknown version is ignored but logged for troubleshooting
                    warn!(
                        version = state.version,
                        "unsupported dnd state version; ignoring persisted value"
                    );
                }
                Ok(None) => {}
                Err(err) => {
                    // Persistence failures must never block daemon startup
                    warn!(?err, "failed to read persisted do-not-disturb state");
                }
            }
        }

        Self {
            // IDs start at 1 to preserve protocol expectations
            next_id: 1,
            dnd_enabled,
            dnd_revision: 0,
            config,
            active: IndexMap::new(),
            history: HistoryStore::new(),
            expirations: HashMap::new(),
            dnd_state_store,
            next_inhibitor_id: 1,
            inhibitors: HashMap::new(),
            inhibited: false,
            inhibitor_count: 0,
        }
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

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

    pub fn inhibited(&self) -> bool {
        self.inhibited
    }

    pub fn inhibitor_count(&self) -> u32 {
        self.inhibitor_count
    }

    pub fn list_active(&self) -> Vec<NotificationView> {
        // Reverse iteration returns newest entries first for panel rendering
        self.active
            .values()
            .rev()
            .map(|notification| notification.to_list_view())
            .collect()
    }

    pub fn list_history(&self) -> Vec<NotificationView> {
        // HistoryStore already returns newest first
        self.history.list_views()
    }

    pub fn active_notification_view(&self, id: u32) -> Option<NotificationView> {
        // Active rows use the richer popup-oriented view because add/update signals
        // are consumed by trusted UIs that may need current image payloads
        self.active
            .get(&id)
            .map(|notification| notification.to_view())
    }

    pub fn history_len(&self) -> usize {
        // Exposed for diagnostics and test assertions
        self.history.len()
    }

    pub fn clear_history(&mut self) {
        // Explicit history wipe used by CLI and control commands
        self.history.clear();
    }
}

#[cfg(test)]
#[path = "store/store_tests/mod.rs"]
mod store_tests;
