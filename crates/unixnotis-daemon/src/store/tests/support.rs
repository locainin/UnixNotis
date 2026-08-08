use std::sync::Arc;
use std::time::Instant;

use unixnotis_core::{Notification, NotificationKey, UiHealth};

use crate::store::{CommitDisposition, InsertOutcome, NotificationStore, PopupAdmission};

impl NotificationStore {
    pub(crate) fn insert(&mut self, notification: Notification, replaces_id: u32) -> InsertOutcome {
        // Store tests use a neutral renderer snapshot unless a case provides one explicitly
        self.insert_with_ui_health(notification, replaces_id, &UiHealth::default())
    }

    pub(crate) fn record_popup_commit_environment(
        &mut self,
        key: NotificationKey,
        admission: PopupAdmission,
        ui_health: &UiHealth,
        popup_hide_after_ms: u64,
    ) {
        self.record_popup_commit_environment_at(
            key,
            admission,
            ui_health,
            popup_hide_after_ms,
            Instant::now(),
        );
    }
}

impl InsertOutcome {
    pub(crate) fn active_notification(&self) -> Arc<Notification> {
        match &self.disposition {
            CommitDisposition::Active(notification) => Arc::clone(notification),
            CommitDisposition::SuppressedDropAll(_) => {
                panic!("active insertion outcome must retain its notification")
            }
        }
    }
}
