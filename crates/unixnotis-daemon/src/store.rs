//! Notification store with ordering and history management.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::Instant;

use indexmap::IndexMap;
use unixnotis_core::{Config, Notification, NotificationView, RuleConfig, Urgency};

/// Mutable notification state owned by the daemon.
pub struct NotificationStore {
    config: Config,
    next_id: u32,
    active: IndexMap<u32, Arc<Notification>>,
    history: HistoryStore,
    expirations: HashMap<u32, Instant>,
    dnd_enabled: bool,
}

pub struct InsertOutcome {
    pub notification: Arc<Notification>,
    pub replaced: bool,
    pub show_popup: bool,
    pub allow_sound: bool,
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

struct HistoryStore {
    entries: HashMap<u32, Arc<Notification>>,
    order: VecDeque<u32>,
}

impl HistoryStore {
    fn new() -> Self {
        Self {
            entries: HashMap::new(),
            order: VecDeque::new(),
        }
    }

    fn len(&self) -> usize {
        self.entries.len()
    }

    fn contains(&self, id: &u32) -> bool {
        self.entries.contains_key(id)
    }

    fn clear(&mut self) {
        self.entries.clear();
        self.order.clear();
    }

    fn list_views(&self) -> Vec<NotificationView> {
        let mut views = Vec::with_capacity(self.entries.len());
        for id in self.order.iter().rev() {
            if let Some(notification) = self.entries.get(id) {
                views.push(notification.to_list_view());
            }
        }
        views
    }

    fn remove(&mut self, id: &u32) -> Option<Arc<Notification>> {
        let removed = self.entries.remove(id);
        if removed.is_some() {
            // Removal is infrequent compared to insertion; pay the cost here to keep order clean.
            self.order.retain(|entry| entry != id);
        }
        removed
    }

    fn insert(&mut self, notification: Arc<Notification>) {
        let id = notification.id;
        if self.entries.contains_key(&id) {
            // Avoid duplicate IDs in order when a notification is replaced.
            self.order.retain(|entry| *entry != id);
        }
        self.entries.insert(id, notification);
        self.order.push_back(id);
    }

    fn evict_to_limit(&mut self, max_entries: usize) {
        while self.entries.len() > max_entries {
            if let Some(id) = self.order.pop_front() {
                if self.entries.remove(&id).is_some() {
                    continue;
                }
            } else {
                break;
            }
        }
    }
}

impl NotificationStore {
    pub fn new(config: Config) -> Self {
        Self {
            next_id: 1,
            dnd_enabled: config.general.dnd_default,
            config,
            active: IndexMap::new(),
            history: HistoryStore::new(),
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
        self.history.list_views()
    }

    pub fn history_len(&self) -> usize {
        self.history.len()
    }

    pub fn insert(&mut self, mut notification: Notification, replaces_id: u32) -> InsertOutcome {
        self.apply_rules(&mut notification);
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
        let stored = Arc::new(notification.to_history());
        self.history.insert(stored);
        self.history
            .evict_to_limit(self.config.history.max_entries);
    }

    fn should_show_popup(&self, notification: &Notification) -> bool {
        if notification.suppress_popup {
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
        if notification.urgency.as_u8() != urgency {
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
        notification.urgency = match force_urgency {
            0 => Urgency::Low,
            2 => Urgency::Critical,
            _ => Urgency::Normal,
        };
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
mod tests {
    use super::contains_ci;

    #[test]
    fn contains_ci_matches_ascii() {
        assert!(contains_ci("Signal-Desktop", "signal"));
        assert!(contains_ci("signal-desktop", "Signal"));
        assert!(!contains_ci("signal-desktop", "brave"));
        assert!(contains_ci("mixedCase", "case"));
        assert!(contains_ci("mixedCase", ""));
    }
}
