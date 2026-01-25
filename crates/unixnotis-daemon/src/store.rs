//! Notification store with ordering, history, and suppression policies.
//!
//! Split into focused modules so persistence and inhibitor tracking stay isolated
//! from the core notification lifecycle logic.

#[path = "store/store_history.rs"]
mod store_history;
#[path = "store/store_inhibit.rs"]
mod store_inhibit;
#[path = "store/store_state.rs"]
mod store_state;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use indexmap::IndexMap;
use tracing::{debug, warn};
use unixnotis_core::{Config, InhibitMode, Notification, NotificationView, RuleConfig, Urgency};

use store_history::HistoryStore;
use store_inhibit::{inhibits_popups, Inhibitor, InhibitorOwnerMismatch};
use store_state::{DndStateStore, DND_STATE_VERSION};

#[cfg(test)]
use std::path::PathBuf;

/// Mutable notification state owned by the daemon.
pub struct NotificationStore {
    config: Config,
    next_id: u32,
    active: IndexMap<u32, Arc<Notification>>,
    history: HistoryStore,
    expirations: HashMap<u32, Instant>,
    dnd_enabled: bool,
    // Optional persistence layer for the DND toggle; absence keeps behavior in-memory only.
    dnd_state_store: Option<DndStateStore>,
    // Monotonic token for inhibitor registration; tokens are never reused in a process.
    next_inhibitor_id: u64,
    // Active inhibitors keyed by token for O(1) lookup and removal.
    inhibitors: HashMap<u64, Inhibitor>,
    // Cached boolean so popup decisions stay O(1) during notify bursts.
    inhibited: bool,
    // Cached total so D-Bus state can be emitted without recomputation.
    inhibitor_count: u32,
}

pub struct InsertOutcome {
    pub notification: Arc<Notification>,
    pub replaced: bool,
    pub show_popup: bool,
    pub allow_sound: bool,
    pub evicted: Vec<u32>,
    pub dropped: bool,
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
        let dnd_state_store = DndStateStore::new();
        Self::new_with_state_store(config, dnd_state_store)
    }

    #[cfg(test)]
    pub(crate) fn new_with_state_dir(config: Config, state_dir: PathBuf) -> Self {
        // Test helper: allow a custom state directory without mutating process env.
        let dnd_state_store = Some(DndStateStore::from_state_dir(state_dir));
        Self::new_with_state_store(config, dnd_state_store)
    }

    fn new_with_state_store(config: Config, dnd_state_store: Option<DndStateStore>) -> Self {
        let mut dnd_enabled = config.general.dnd_default;
        if let Some(store) = dnd_state_store.as_ref() {
            match store.load() {
                Ok(Some(state)) if state.version == DND_STATE_VERSION => {
                    dnd_enabled = state.dnd_enabled;
                    debug!(
                        dnd_enabled,
                        "loaded persisted do-not-disturb state"
                    );
                }
                Ok(Some(state)) => {
                    warn!(
                        version = state.version,
                        "unsupported dnd state version; ignoring persisted value"
                    );
                }
                Ok(None) => {}
                Err(err) => {
                    warn!(?err, "failed to read persisted do-not-disturb state");
                }
            }
        }

        Self {
            next_id: 1,
            dnd_enabled,
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

    pub fn set_dnd(&mut self, enabled: bool) -> bool {
        if self.dnd_enabled == enabled {
            return false;
        }
        self.dnd_enabled = enabled;
        if let Some(store) = self.dnd_state_store.as_ref() {
            if let Err(err) = store.persist(enabled) {
                warn!(?err, "failed to persist do-not-disturb state");
            }
        }
        true
    }

    pub fn inhibited(&self) -> bool {
        self.inhibited
    }

    pub fn inhibitor_count(&self) -> u32 {
        self.inhibitor_count
    }

    pub fn add_inhibitor(&mut self, owner: String, reason: String, scope: u32) -> u64 {
        let id = self.next_inhibitor_id.max(1);
        self.next_inhibitor_id = id.saturating_add(1);
        // Keep the owner name so disconnect cleanup can evict all related inhibitors.
        self.inhibitors.insert(
            id,
            Inhibitor {
                id,
                owner,
                reason,
                scope,
            },
        );
        self.refresh_inhibit_state();
        id
    }

    pub fn remove_inhibitor(
        &mut self,
        id: u64,
        owner: &str,
    ) -> Result<bool, InhibitorOwnerMismatch> {
        let Some(existing) = self.inhibitors.get(&id) else {
            return Ok(false);
        };
        if existing.owner != owner {
            // Owner checks prevent one client from removing another client's inhibitor.
            return Err(InhibitorOwnerMismatch::new(
                existing.owner.clone(),
                owner.to_string(),
            ));
        }
        self.inhibitors.remove(&id);
        self.refresh_inhibit_state();
        Ok(true)
    }

    pub fn remove_inhibitors_by_owner(&mut self, owner: &str) -> bool {
        let before = self.inhibitors.len();
        self.inhibitors.retain(|_, inhibitor| inhibitor.owner != owner);
        if self.inhibitors.len() == before {
            return false;
        }
        // Refresh cached counters only when the set actually changes.
        self.refresh_inhibit_state();
        true
    }

    pub fn list_inhibitors(&self) -> Vec<(u64, String, u32, String)> {
        let mut inhibitors = Vec::with_capacity(self.inhibitors.len());
        for inhibitor in self.inhibitors.values() {
            inhibitors.push((
                inhibitor.id,
                inhibitor.reason.clone(),
                inhibitor.scope,
                inhibitor.owner.clone(),
            ));
        }
        // Sort by token for deterministic CLI output and test expectations.
        inhibitors.sort_by_key(|(id, _, _, _)| *id);
        inhibitors
    }

    pub fn list_active(&self) -> Vec<NotificationView> {
        self.active
            .values()
            .rev()
            .map(|notification| notification.to_list_view())
            .collect()
    }

    pub fn list_history(&self) -> Vec<NotificationView> {
        self.history.list_views()
    }

    pub fn history_len(&self) -> usize {
        self.history.len()
    }

    pub fn insert(&mut self, mut notification: Notification, replaces_id: u32) -> InsertOutcome {
        self.apply_rules(&mut notification);
        if self.should_drop_inhibited() {
            let assigned_id = self.next_id();
            notification.id = assigned_id;
            let notification = Arc::new(notification);
            return InsertOutcome {
                show_popup: false,
                allow_sound: false,
                notification,
                replaced: false,
                evicted: Vec::new(),
                dropped: true,
            };
        }
        // Preserve protocol semantics: replaces_id only applies when it matches an existing item.
        let has_replaces_id = replaces_id != 0;
        // Replacement is only true when the referenced notification is present.
        let replaced = has_replaces_id
            && (self.active.contains_key(&replaces_id) || self.history.contains(&replaces_id));
        let assigned_id = if replaced {
            replaces_id
        } else {
            self.next_id()
        };
        notification.id = assigned_id;

        // Remove any stale entries for this ID before inserting the replacement.
        self.active.shift_remove(&assigned_id);
        self.history.remove(&assigned_id);
        self.expirations.remove(&assigned_id);

        let notification = Arc::new(notification);
        self.active.insert(assigned_id, notification.clone());
        let evicted = self.enforce_active_limit();

        InsertOutcome {
            show_popup: self.should_show_popup(&notification),
            allow_sound: self.should_play_sound(&notification),
            notification,
            replaced,
            evicted,
            dropped: false,
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

        let removed_history = self.history.remove(&id).is_some();

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
        // Walk the u32 space (skipping 0) to find a free id; wrap around once to avoid loops.
        let start = self.next_id.max(1);
        let mut candidate = start;
        loop {
            if !self.active.contains_key(&candidate) && !self.history.contains(&candidate) {
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
            // max_active == 0 means "no active list"; archive everything immediately.
            let mut evicted = Vec::new();
            // shift_remove_index(0) evicts the oldest entry while preserving insertion order.
            while let Some((id, notification)) = self.active.shift_remove_index(0) {
                self.expirations.remove(&id);
                self.push_history(notification);
                evicted.push(id);
            }
            return evicted;
        }
        let mut evicted = Vec::new();
        while self.active.len() > max_active {
            // shift_remove_index(0) evicts the oldest entry while preserving insertion order.
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
        if self.config.history.max_entries == 0 {
            // Honor zero-history limit without allocating a history copy.
            self.history.clear();
            return;
        }
        if notification.is_transient && !self.config.history.transient_to_history {
            return;
        }
        let stored = Arc::new(notification.to_history());
        self.history.insert(stored);
        self.history.evict_to_limit(self.config.history.max_entries);
    }

    fn should_show_popup(&self, notification: &Notification) -> bool {
        if notification.suppress_popup {
            return false;
        }
        if self.inhibited {
            return false;
        }
        if self.dnd_enabled {
            return notification.urgency == Urgency::Critical;
        }
        true
    }

    fn should_play_sound(&self, notification: &Notification) -> bool {
        if notification.suppress_sound {
            return false;
        }
        // Inhibitors only gate popup rendering; sound follows DND and rule overrides.
        if self.dnd_enabled {
            return notification.urgency == Urgency::Critical;
        }
        true
    }

    fn apply_rules(&self, notification: &mut Notification) {
        for rule in &self.config.rules {
            if !rule_matches(rule, notification) {
                continue;
            }
            apply_rule(rule, notification);
        }
    }

    fn should_drop_inhibited(&self) -> bool {
        self.inhibited && matches!(self.config.inhibit.mode, InhibitMode::DropAll)
    }

    fn refresh_inhibit_state(&mut self) {
        self.inhibitor_count = self.inhibitors.len() as u32;
        self.inhibited = self
            .inhibitors
            .values()
            .any(|inhibitor| inhibits_popups(inhibitor.scope));
    }
}

fn rule_matches(rule: &RuleConfig, notification: &Notification) -> bool {
    if let Some(app) = rule.app.as_ref() {
        if !contains_ci(&notification.app_name, app) {
            return false;
        }
    }
    if let Some(summary) = rule.summary.as_ref() {
        if !contains_ci(&notification.summary, summary) {
            return false;
        }
    }
    if let Some(body) = rule.body.as_ref() {
        if !contains_ci(&notification.body, body) {
            return false;
        }
    }
    if let Some(category) = rule.category.as_ref() {
        match notification.category.as_ref() {
            Some(value) if contains_ci(value, category) => {}
            _ => return false,
        }
    }
    if let Some(urgency) = rule.urgency {
        if notification.urgency != Urgency::from(urgency) {
            return false;
        }
    }
    true
}

fn apply_rule(rule: &RuleConfig, notification: &mut Notification) {
    if let Some(no_popup) = rule.no_popup {
        notification.suppress_popup = no_popup;
    }
    if let Some(silent) = rule.silent {
        notification.suppress_sound = silent;
    }
    if let Some(force_urgency) = rule.force_urgency {
        notification.urgency = Urgency::from(force_urgency);
    }
    if let Some(expire_timeout_ms) = rule.expire_timeout_ms {
        let clamped = expire_timeout_ms.clamp(i32::MIN as i64, i32::MAX as i64) as i32;
        notification.expire_timeout = clamped;
    }
    if let Some(resident) = rule.resident {
        notification.is_resident = resident;
    }
    if let Some(transient) = rule.transient {
        notification.is_transient = transient;
    }
}

fn contains_ci(haystack: &str, needle: &str) -> bool {
    // ASCII-only case-insensitive substring match without per-call allocations.
    if needle.is_empty() {
        return true;
    }
    let haystack_bytes = haystack.as_bytes();
    let needle_bytes = needle.as_bytes();
    if needle_bytes.len() > haystack_bytes.len() {
        return false;
    }
    haystack_bytes
        .windows(needle_bytes.len())
        .any(|window| window.eq_ignore_ascii_case(needle_bytes))
}

#[cfg(test)]
#[path = "store/store_tests.rs"]
mod store_tests;
