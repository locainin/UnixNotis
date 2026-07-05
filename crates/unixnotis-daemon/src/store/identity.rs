use tracing::warn;
use unixnotis_core::Notification;

use super::NotificationStore;

impl NotificationStore {
    pub fn is_notification_owned_by(
        &self,
        id: u32,
        sender: &str,
        sender_pid: Option<u32>,
        sender_start_time: Option<u64>,
    ) -> bool {
        // Ownership checks are valid only against active notifications
        let Some(notification) = self.active.get(&id) else {
            return false;
        };
        notification_is_owned_by(notification, Some(sender), sender_pid, sender_start_time)
    }

    pub(super) fn next_id(&mut self) -> u32 {
        // Search at most active+history+1 IDs, which guarantees one free slot in that window
        let start = self.next_id.max(1);
        let mut candidate = start;
        let used = self.active.len().saturating_add(self.history.len());
        let max_attempts = used.saturating_add(1).max(1);
        for _ in 0..max_attempts {
            // Candidate must be absent from both active and history sets
            if !self.active.contains_key(&candidate) && !self.history.contains(&candidate) {
                self.next_id = candidate.wrapping_add(1);
                if self.next_id == 0 {
                    self.next_id = 1;
                }
                return candidate;
            }
            // wrapping_add keeps progress valid even near u32::MAX
            candidate = candidate.wrapping_add(1);
            if candidate == 0 {
                candidate = 1;
            }
        }
        warn!(
            used,
            "notification id space exhausted; reusing id to avoid deadlock"
        );
        self.next_id = start.wrapping_add(1);
        if self.next_id == 0 {
            self.next_id = 1;
        }
        start
    }

    pub(super) fn can_replace_notification_for_sender(
        &self,
        id: u32,
        sender: Option<&str>,
        sender_pid: Option<u32>,
        sender_start_time: Option<u64>,
    ) -> bool {
        // Replacement is allowed only for the sender that owns the original notification
        let Some(existing) = self.active.get(&id).or_else(|| self.history.get(&id)) else {
            return false;
        };
        notification_is_owned_by(existing, sender, sender_pid, sender_start_time)
    }
}

pub(super) fn notification_is_owned_by(
    notification: &Notification,
    sender: Option<&str>,
    sender_pid: Option<u32>,
    sender_start_time: Option<u64>,
) -> bool {
    // Exact bus-name match is the strongest ownership proof on the session bus
    match (sender, notification.sender_name.as_deref()) {
        (Some(caller), Some(owner)) if caller == owner => return true,
        _ => {}
    }
    // Process-lifetime match keeps reconnect support without trusting pid reuse alone
    if let (Some(caller_pid), Some(owner_pid), Some(caller_start), Some(owner_start)) = (
        sender_pid,
        notification.sender_pid,
        sender_start_time,
        notification.sender_start_time,
    ) {
        if caller_pid == owner_pid && caller_start == owner_start {
            return true;
        }
    }
    false
}
