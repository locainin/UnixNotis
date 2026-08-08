//! Shared notification and persistence fixtures for store tests

use std::collections::HashMap;

use chrono::Utc;
use unixnotis_core::{Config, Notification, NotificationImage, Urgency};
use zbus::zvariant::OwnedValue;

use super::dnd::persistence::{PersistedDndState, DND_STATE_FILE};
use super::dnd::DndStateStore;
use super::model::NotificationStore;

impl NotificationStore {
    pub(crate) fn new_with_state_dir(config: Config, state_dir: std::path::PathBuf) -> Self {
        // Isolated persistence roots keep tests away from the live XDG state directory
        let state_store = Some(DndStateStore::from_state_dir(state_dir));
        Self::new_with_state_store(config, state_store)
    }
}

pub(in crate::store) fn make_notification(summary: &str) -> Notification {
    Notification {
        id: 0,
        generation: 0,
        app_name: "TestApp".to_string(),
        app_icon: String::new(),
        attribution: unixnotis_core::NotificationAttribution::verified(
            "TestApp",
            "TestApp",
            "org.example.TestApp",
            "",
            unixnotis_core::AttributionReason::ExactSystemExecutable,
            "authenticated test fixture",
            "test:verified:test-app".to_string(),
        ),
        attribution_diagnostics: unixnotis_core::AttributionDiagnostics::default(),
        summary: summary.to_string(),
        body: String::new(),
        actions: Vec::new(),
        inline_reply: unixnotis_core::InlineReply::default(),
        inline_reply_policy: unixnotis_core::InlineReplyPolicy::Allow,
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

pub fn make_notification_with_sender(
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

pub(in crate::store) fn make_store_with_limits(
    max_active: usize,
    max_entries: usize,
) -> NotificationStore {
    let mut config = Config::default();
    // Test helper uses explicit limits so each case isolates one policy branch
    config.history.max_active = max_active;
    config.history.max_entries = max_entries;
    NotificationStore::new(config)
}

pub(in crate::store) fn make_temp_state_dir(label: &str) -> std::path::PathBuf {
    let mut path = std::env::temp_dir();
    let pid = std::process::id();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    path.push(format!("unixnotis-test-{label}-{pid}-{nanos}"));
    std::fs::create_dir_all(&path).expect("create temp state dir");
    path
}

pub(in crate::store) fn write_dnd_state(dir: &std::path::Path, enabled: bool, version: u32) {
    let state = PersistedDndState {
        version,
        dnd_enabled: enabled,
        expires_at: None,
        updated_at: Some("2025-01-01T00:00:00Z".to_string()),
    };
    let payload = serde_json::to_string(&state).expect("serialize state");
    let path = dir.join("unixnotis").join(DND_STATE_FILE);
    std::fs::create_dir_all(path.parent().expect("state parent")).expect("create state directory");
    std::fs::write(&path, payload).expect("write state");
}

pub(in crate::store) fn cleanup_temp_dir(dir: &std::path::Path) {
    let _ = std::fs::remove_dir_all(dir);
}

pub(in crate::store) fn apply_dnd_update(store: &mut NotificationStore, enabled: bool) -> bool {
    let write = store.set_dnd(enabled);
    if let Some(state_store) = write.persist.as_ref() {
        state_store
            .persist(write.current, write.current_expires_at)
            .expect("persist dnd state");
    }
    write.changed
}
