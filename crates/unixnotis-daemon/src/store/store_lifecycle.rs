use std::sync::Arc;
use std::time::Instant;

use unixnotis_core::{
    popup_allowed_by_state, should_archive_closed_notification, CloseReason, ControlState,
    Notification, Urgency,
};

use super::{DismissOutcome, InsertOutcome, NotificationStore};

// Hard ceiling for concurrently active notifications to protect panel/popups stability.
const ACTIVE_HARD_CAP: usize = 12;

impl NotificationStore {
    pub fn insert(&mut self, mut notification: Notification, replaces_id: u32) -> InsertOutcome {
        // Rule transforms happen before any storage decision
        self.apply_rules(&mut notification);
        if self.should_drop_inhibited() {
            // DropAll mode still assigns an ID so call sites can log consistent metadata
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

        // replaces_id is valid only when it points to an existing, owned notification
        let has_replaces_id = replaces_id != 0;
        let replaced = has_replaces_id
            && self.can_replace_notification_for_sender(
                replaces_id,
                notification.sender_name.as_deref(),
                notification.sender_pid,
                notification.sender_start_time,
            );
        // Replacement preserves ID only when sender ownership is confirmed
        let assigned_id = if replaced {
            replaces_id
        } else {
            self.next_id()
        };
        notification.id = assigned_id;

        // Drop stale copies before inserting the fresh one
        self.active.shift_remove(&assigned_id);
        self.history.remove(&assigned_id);
        self.expirations.remove(&assigned_id);

        let notification = Arc::new(notification);
        // Active map keeps insertion order so oldest eviction is deterministic
        self.active.insert(assigned_id, notification.clone());
        // Enforce active cap immediately so UI never sees oversized active sets
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

    pub fn close(&mut self, id: u32, reason: CloseReason) -> Option<Arc<Notification>> {
        // Active removal and expiration cleanup always happen together
        let removed = self.active.shift_remove(&id);
        self.expirations.remove(&id);
        if let Some(notification) = removed.clone() {
            // Closed rows and panel rows should follow the same archive rule
            self.push_history(notification.clone(), reason);
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

    fn enforce_active_limit(&mut self) -> Vec<u32> {
        // Config limit still applies, but active list never exceeds the global safety cap.
        let max_active = self.config.history.max_active.min(ACTIVE_HARD_CAP);
        if max_active == 0 {
            // max_active=0 means archive everything immediately
            let mut evicted = Vec::new();
            while let Some((id, notification)) = self.active.shift_remove_index(0) {
                // Evicted notifications should not retain pending expiration entries
                self.expirations.remove(&id);
                // Active-cap eviction behaves like a daemon-side close for history policy
                self.push_history(notification, CloseReason::Undefined);
                evicted.push(id);
            }
            return evicted;
        }

        let mut evicted = Vec::new();
        while self.active.len() > max_active {
            // remove_index(0) always pops the oldest notification first
            if let Some((id, notification)) = self.active.shift_remove_index(0) {
                // Eviction path mirrors close path so state stays consistent
                self.expirations.remove(&id);
                // Evicted rows still need the same archive rule as any other close
                self.push_history(notification, CloseReason::Undefined);
                evicted.push(id);
            } else {
                // Defensive break for impossible map/index mismatch cases
                break;
            }
        }
        evicted
    }

    fn push_history(&mut self, notification: Arc<Notification>, reason: CloseReason) {
        if self.config.history.max_entries == 0 {
            // Clear keeps memory bounded when history feature is disabled
            self.history.clear();
            return;
        }
        // One shared archive rule keeps daemon and center close handling aligned
        if !should_archive_closed_notification(
            reason,
            notification.is_transient,
            self.config.history.transient_to_history,
        ) {
            return;
        }
        // to_history strips non-history-only fields and keeps stored payload compact
        let stored = Arc::new(notification.to_history());
        self.history.insert(stored);
        self.history.evict_to_limit(self.config.history.max_entries);
    }

    fn should_show_popup(&self, notification: &Notification) -> bool {
        // Rule-level popup suppression is highest priority
        if notification.suppress_popup {
            return false;
        }
        // Shared gate keeps daemon admission aligned with popup-side cleanup
        popup_allowed_by_state(
            notification.urgency as u8,
            &ControlState {
                dnd_enabled: self.dnd_enabled,
                history_count: 0,
                inhibited: self.inhibited,
                inhibitor_count: self.inhibitor_count,
            },
        )
    }

    fn should_play_sound(&self, notification: &Notification) -> bool {
        // Rule-level silence always wins
        if notification.suppress_sound {
            return false;
        }
        // Inhibitors should suppress sound too so focus/presentation mode stays quiet.
        if self.inhibited {
            return false;
        }
        // DND still keeps critical notification sounds enabled.
        if self.dnd_enabled {
            return notification.urgency == Urgency::Critical;
        }
        true
    }
}
