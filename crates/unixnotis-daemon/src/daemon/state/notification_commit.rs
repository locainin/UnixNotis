//! Serialized notification generation commits

use unixnotis_core::Notification;

use crate::expire::ExpirationScheduler;
use crate::store::{CommitDisposition, InsertOutcome};

use super::DaemonState;

impl DaemonState {
    pub(in crate::daemon) async fn commit_notification_generation(
        &self,
        notification: Notification,
        replaces_id: u32,
        scheduler: &ExpirationScheduler,
    ) -> InsertOutcome {
        // Every nonzero replacement request shares the ID gate with actions and inline replies
        let _interaction = if replaces_id == 0 {
            None
        } else {
            Some(self.interaction_gates.lock(replaces_id).await)
        };
        let mut store = self.store.lock().await;
        let outcome = store.insert_with_ui_health(notification, replaces_id, &self.ui_health());
        if let CommitDisposition::Active(notification) = &outcome.disposition {
            // The committed generation and its expiration ticket become visible together
            store.set_expiration(notification, outcome.expiration);
            scheduler.schedule(notification.id, notification.generation, outcome.expiration);
        }
        for key in &outcome.evicted {
            // Eviction cancels the exact generation removed by the same commit
            scheduler.schedule(key.id, key.generation, None);
        }
        outcome
    }
}
