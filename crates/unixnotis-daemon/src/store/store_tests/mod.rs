//! Store regression coverage and persistence validation

use super::store_state::{PersistedDndState, DND_STATE_FILE, DND_STATE_VERSION};
use super::{contains_ci, NotificationStore};
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use unixnotis_core::{CloseReason, Config, InhibitMode, Notification, NotificationImage, Urgency};
use zbus::zvariant::OwnedValue;

mod dnd;
mod inhibit;
mod lifecycle;
mod ownership;
mod rules;

pub(super) fn make_notification(summary: &str) -> Notification {
    Notification {
        id: 0,
        app_name: "TestApp".to_string(),
        app_icon: String::new(),
        summary: summary.to_string(),
        body: String::new(),
        actions: Vec::new(),
        hints: HashMap::<String, OwnedValue>::new(),
        urgency: Urgency::Normal,
        category: None,
        is_transient: false,
        is_resident: false,
        suppress_popup: false,
        suppress_sound: false,
        image: NotificationImage::default(),
        expire_timeout: 0,
        received_at: Utc::now(),
        sender_name: Some(":1.test".to_string()),
        sender_pid: Some(1234),
        sender_start_time: Some(555),
        sender_executable: Some("/usr/bin/test-app".to_string()),
    }
}

pub(super) fn make_notification_with_sender(
    summary: &str,
    sender: &str,
    pid: u32,
    start_time: u64,
) -> Notification {
    let mut notification = make_notification(summary);
    notification.sender_name = Some(sender.to_string());
    notification.sender_pid = Some(pid);
    notification.sender_start_time = Some(start_time);
    notification
}

pub(super) fn make_store_with_limits(max_active: usize, max_entries: usize) -> NotificationStore {
    let mut config = Config::default();
    // Test helper uses explicit limits so each case isolates one policy branch
    config.history.max_active = max_active;
    config.history.max_entries = max_entries;
    NotificationStore::new(config)
}

pub(super) fn make_temp_state_dir(label: &str) -> std::path::PathBuf {
    let mut path = std::env::temp_dir();
    let pid = std::process::id();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    path.push(format!("unixnotis-test-{label}-{pid}-{nanos}"));
    std::fs::create_dir_all(&path).expect("create temp state dir");
    path
}

pub(super) fn write_dnd_state(dir: &std::path::Path, enabled: bool, version: u32) {
    let state = PersistedDndState {
        version,
        dnd_enabled: enabled,
        updated_at: Some("2025-01-01T00:00:00Z".to_string()),
    };
    let payload = serde_json::to_string(&state).expect("serialize state");
    let path = dir.join("unixnotis").join(DND_STATE_FILE);
    std::fs::create_dir_all(path.parent().expect("state parent")).expect("create state directory");
    std::fs::write(&path, payload).expect("write state");
}

pub(super) fn cleanup_temp_dir(dir: &std::path::Path) {
    let _ = std::fs::remove_dir_all(dir);
}

pub(super) fn apply_dnd_update(store: &mut NotificationStore, enabled: bool) -> bool {
    let write = store.set_dnd(enabled);
    if let Some(state_store) = write.persist.as_ref() {
        state_store
            .persist(write.current)
            .expect("persist dnd state");
    }
    write.changed
}
