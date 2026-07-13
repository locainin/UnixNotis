use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use indexmap::IndexMap;
use unixnotis_core::{Config, Notification};

use super::{DndStateStore, HistoryStore, Inhibitor};

/// Mutable notification state owned by the daemon
pub struct NotificationStore {
    // Immutable runtime config snapshot
    pub(super) config: Config,
    // Next candidate id for allocation
    pub(super) next_id: u32,
    // Active notifications in insertion order
    pub(super) active: IndexMap<u32, Arc<Notification>>,
    // Archived notifications with bounded retention
    pub(super) history: HistoryStore,
    // Optional expiration deadline per active id
    pub(super) expirations: HashMap<u32, Instant>,
    // Effective DND switch after loading persisted state
    pub(super) dnd_enabled: bool,
    // Monotonic in-memory revision for DND writes
    pub(super) dnd_revision: u64,
    // Optional persistence layer for DND; absent store keeps behavior in-memory
    pub(super) dnd_state_store: Option<DndStateStore>,
    // Token counter for inhibitors; never reused in a process
    pub(super) next_inhibitor_id: u64,
    // Active inhibitors keyed by token for quick lookup/removal
    pub(super) inhibitors: HashMap<u64, Inhibitor>,
    // Cached flags avoid rescanning inhibitors on every notification
    pub(super) inhibited: bool,
    pub(super) inhibitor_count: u32,
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

pub struct DndWrite {
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
    pub const fn removed_any(&self) -> bool {
        // Convenience helper for callers that only need yes/no
        self.removed_active || self.removed_history
    }
}
