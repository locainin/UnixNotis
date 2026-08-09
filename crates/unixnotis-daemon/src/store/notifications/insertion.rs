use std::sync::Arc;

use unixnotis_core::{
    popup_allowed_by_state, should_archive_closed_notification, CloseReason, ControlState,
    Notification, NotificationKey, UiHealth, Urgency,
};

use crate::store::{
    CommitDisposition, InsertOutcome, NotificationStore, PopupAdmission, PopupSuppressionReason,
    StableProcessIdentity, SuppressedNotification,
};

use super::timeout::resolve_timeout_policy;

// Each resolved sender principal receives an isolated active-state budget
const ACTIVE_PER_PRINCIPAL_HARD_CAP: usize = 12;
// The emergency ceiling remains large enough that one normal sender cannot displace another
const ABSOLUTE_ACTIVE_HARD_CAP: usize = 128;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum ActivePrincipal {
    Stable(StableProcessIdentity),
    BusName(zbus::names::OwnedUniqueName),
    Unknown,
}

impl NotificationStore {
    pub fn insert_with_ui_health(
        &mut self,
        mut notification: Notification,
        replaces_id: u32,
        ui_health: &UiHealth,
    ) -> InsertOutcome {
        // Rule transforms happen before any storage decision
        self.apply_rules(&mut notification);
        let timeout_policy = resolve_timeout_policy(&self.config, &notification);
        if self.should_drop_inhibited() {
            // DropAll discards notification content, not protocol lifecycle
            // Only process-lifetime identity survives long enough to close the returned ID
            let assigned_id = self.next_id();
            let generation = self.next_generation;
            self.next_generation = self
                .next_generation
                .checked_add(1)
                .expect("notification generation space must not be exhausted");
            let owner = notification
                .sender_pid
                .zip(notification.sender_start_time)
                .map(|(pid, start_time)| StableProcessIdentity { pid, start_time });
            return InsertOutcome {
                popup_admission: PopupAdmission::Suppressed(
                    PopupSuppressionReason::DropAllInhibitor,
                ),
                allow_sound: false,
                disposition: CommitDisposition::SuppressedDropAll(SuppressedNotification {
                    id: assigned_id,
                    generation,
                    owner,
                }),
                replaced: false,
                evicted: Vec::new(),
                expiration: None,
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
        // A replacement keeps its protocol ID but always receives a fresh commit identity
        notification.generation = self.next_generation;
        self.next_generation = self
            .next_generation
            .checked_add(1)
            .expect("notification generation space must not be exhausted");

        // Drop stale copies before inserting the fresh one
        self.active.shift_remove(&assigned_id);
        self.history.remove(&assigned_id);
        self.expirations.remove(&assigned_id);
        self.popup_decisions
            .retain(|key, _decision| key.id != assigned_id);
        self.popup_timings
            .retain(|key, _timing| key.id != assigned_id);

        let admitted_at = std::time::Instant::now();
        let expiration = timeout_policy
            .active_close_after
            // Overflow disables automatic expiration instead of reversing it into immediate close
            .and_then(|duration| admitted_at.checked_add(duration));
        let notification = Arc::new(notification);
        // Active map keeps insertion order so principal-local eviction is deterministic
        self.active.insert(assigned_id, notification.clone());
        // Replacement already removed its previous generation and therefore consumes one slot
        let evicted = self.enforce_active_limits(active_principal(&notification));

        let popup_admission = self.popup_admission(&notification);
        self.record_popup_commit_environment_at(
            notification.key(),
            popup_admission,
            ui_health,
            timeout_policy.popup_hide_after_ms,
            admitted_at,
        );
        InsertOutcome {
            popup_admission,
            allow_sound: self.should_play_sound(&notification),
            disposition: CommitDisposition::Active(notification),
            replaced,
            evicted,
            expiration,
        }
    }

    fn enforce_active_limits(&mut self, admitted: ActivePrincipal) -> Vec<NotificationKey> {
        let per_principal_limit = self
            .config
            .history
            .max_active
            .min(ACTIVE_PER_PRINCIPAL_HARD_CAP);
        let mut evicted = Vec::new();
        while self.active_count_for(&admitted) > per_principal_limit {
            // A sender over its budget can remove only that sender's oldest active generation
            let Some(key) = self.evict_oldest_for_principal(&admitted) else {
                break;
            };
            evicted.push(key);
        }

        while self.active.len() > ABSOLUTE_ACTIVE_HARD_CAP {
            // At the emergency boundary, the largest consumer yields first
            // Equal shares prefer the newly admitted principal so established clients stay intact
            let victim = self
                .largest_active_principal(&admitted)
                .unwrap_or_else(|| admitted.clone());
            let Some(key) = self.evict_oldest_for_principal(&victim) else {
                break;
            };
            evicted.push(key);
        }
        evicted
    }

    fn active_count_for(&self, principal: &ActivePrincipal) -> usize {
        self.active
            .values()
            .filter(|notification| &active_principal(notification) == principal)
            .count()
    }

    fn largest_active_principal(&self, admitted: &ActivePrincipal) -> Option<ActivePrincipal> {
        let mut counts = std::collections::HashMap::new();
        for notification in self.active.values() {
            let count = counts
                .entry(active_principal(notification))
                .or_insert(0usize);
            *count = count.saturating_add(1);
        }
        let admitted_count = counts.get(admitted).copied().unwrap_or(0);
        let largest = counts.values().copied().max()?;
        if admitted_count == largest {
            return Some(admitted.clone());
        }
        counts
            .into_iter()
            .find_map(|(principal, count)| (count == largest).then_some(principal))
    }

    fn evict_oldest_for_principal(
        &mut self,
        principal: &ActivePrincipal,
    ) -> Option<NotificationKey> {
        let index = self
            .active
            .values()
            .position(|notification| &active_principal(notification) == principal)?;
        let (id, notification) = self.active.shift_remove_index(index)?;
        let key = notification.key();
        self.expirations.remove(&id);
        self.push_history(notification, CloseReason::Undefined);
        Some(key)
    }

    pub(super) fn push_history(&mut self, notification: Arc<Notification>, reason: CloseReason) {
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
        // Keep only weak source identity alongside the compact history payload
        let source = Arc::downgrade(&notification);
        // to_history strips non-history-only fields and keeps stored payload compact
        let stored = Arc::new(notification.to_history());
        let id = stored.id;
        self.history.insert(stored);
        self.history.set_source(id, source);
        self.history.evict_to_limit(self.config.history.max_entries);
    }

    pub(crate) fn popup_admission(&self, notification: &Notification) -> PopupAdmission {
        // Rule-level popup suppression is highest priority
        if notification.suppress_popup {
            return PopupAdmission::Suppressed(PopupSuppressionReason::Rule);
        }
        if self.inhibited {
            return PopupAdmission::Suppressed(PopupSuppressionReason::Inhibitor);
        }
        // Shared gate keeps daemon admission aligned with popup-side cleanup
        if popup_allowed_by_state(
            notification.urgency as u8,
            &ControlState {
                dnd_enabled: self.dnd_enabled,
                dnd_expires_at: self.dnd_expires_at.unwrap_or(0),
                history_count: 0,
                inhibited: self.inhibited,
                inhibitor_count: self.inhibitor_count,
            },
        ) {
            PopupAdmission::Show
        } else {
            PopupAdmission::Suppressed(PopupSuppressionReason::Dnd)
        }
    }

    fn should_play_sound(&self, notification: &Notification) -> bool {
        // Rule-level silence always wins
        if notification.suppress_sound {
            return false;
        }
        // Inhibitors should suppress sound too so focus/presentation mode stays quiet
        if self.inhibited {
            return false;
        }
        // DND still keeps critical notification sounds enabled
        if self.dnd_enabled {
            return notification.urgency == Urgency::Critical;
        }
        true
    }
}

fn active_principal(notification: &Notification) -> ActivePrincipal {
    if let Some((pid, start_time)) = notification.sender_pid.zip(notification.sender_start_time) {
        return ActivePrincipal::Stable(StableProcessIdentity { pid, start_time });
    }
    // A unique bus address is weaker than process identity but still isolates live connections
    notification
        .sender_name
        .as_deref()
        .and_then(|sender| zbus::names::OwnedUniqueName::try_from(sender).ok())
        .map_or(ActivePrincipal::Unknown, ActivePrincipal::BusName)
}
