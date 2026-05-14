//! Notification records and their lightweight UI views.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use zbus::zvariant::{OwnedValue, Type};

use super::image::NotificationImage;
use super::types::{Action, Urgency};

/// Full notification record stored by the daemon.
#[derive(Debug)]
pub struct Notification {
    // Stable identifier assigned by the daemon.
    pub id: u32,
    // Origin metadata for display and filtering.
    pub app_name: String,
    pub app_icon: String,
    // User-facing content as provided by the sender.
    pub summary: String,
    pub body: String,
    // Optional actions supplied by the app.
    pub actions: Vec<Action>,
    // Raw hints preserved for storage and downstream consumers.
    pub hints: HashMap<String, OwnedValue>,
    // Derived urgency used for styling and escalation.
    pub urgency: Urgency,
    pub category: Option<String>,
    // Flags from the notification protocol.
    pub is_transient: bool,
    pub is_resident: bool,
    /// Suppress showing this notification as a popup.
    pub suppress_popup: bool,
    /// Suppress sound playback for this notification.
    pub suppress_sound: bool,
    pub image: NotificationImage,
    pub expire_timeout: i32,
    pub received_at: DateTime<Utc>,
    // D-Bus unique sender name for ownership checks in daemon-side operations.
    pub sender_name: Option<String>,
    // Sender process metadata is retained for diagnostics and audit logging.
    pub sender_pid: Option<u32>,
    pub sender_start_time: Option<u64>,
    pub sender_executable: Option<String>,
}

impl Notification {
    /// Convert to a lightweight view for UI consumption.
    pub fn to_view(&self) -> NotificationView {
        NotificationView {
            id: self.id,
            app_name: self.app_name.clone(),
            summary: self.summary.clone(),
            body: self.body.clone(),
            actions: self.actions.clone(),
            urgency: self.urgency.as_u8(),
            // Center and popup policy both need the transient bit to stay in sync
            is_transient: self.is_transient,
            // UIs only need the text, actions, and image payload used for rendering
            image: self.image.clone(),
            // Protocol flags and sender metadata stay daemon-side to keep D-Bus payloads small
        }
    }

    /// Convert to a view for list rows with heavy image data removed.
    pub fn to_list_view(&self) -> NotificationView {
        NotificationView {
            id: self.id,
            app_name: self.app_name.clone(),
            summary: self.summary.clone(),
            body: self.body.clone(),
            actions: self.actions.clone(),
            urgency: self.urgency.as_u8(),
            // History policy still depends on the transient bit in panel rows
            is_transient: self.is_transient,
            // List rows should avoid carrying raw image buffers across D-Bus
            image: self.image.for_listing(),
            // Protocol flags and sender metadata stay daemon-side to keep D-Bus payloads small
        }
    }

    /// Create a history entry with heavyweight hint data stripped out.
    pub fn to_history(&self) -> Notification {
        // History entries should never retain raw image-data blobs.
        let mut image = self.image.clone();
        image.has_image_data = false;
        image.image_data = Default::default();
        Notification {
            id: self.id,
            app_name: self.app_name.clone(),
            app_icon: self.app_icon.clone(),
            summary: self.summary.clone(),
            body: self.body.clone(),
            actions: self.actions.clone(),
            // Keep history entries lightweight by dropping raw hint payloads.
            hints: HashMap::new(),
            urgency: self.urgency,
            category: self.category.clone(),
            is_transient: self.is_transient,
            is_resident: self.is_resident,
            suppress_popup: self.suppress_popup,
            suppress_sound: self.suppress_sound,
            image,
            expire_timeout: self.expire_timeout,
            received_at: self.received_at,
            sender_name: self.sender_name.clone(),
            sender_pid: self.sender_pid,
            sender_start_time: self.sender_start_time,
            sender_executable: self.sender_executable.clone(),
        }
    }
}

/// Serializable view of a notification for D-Bus signals.
#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq)]
pub struct NotificationView {
    // Identifier matches Notification::id.
    pub id: u32,
    // Lightweight fields used for UI display and filtering.
    // Intentionally omits daemon-only protocol flags and timestamps
    pub app_name: String,
    pub summary: String,
    pub body: String,
    pub actions: Vec<Action>,
    pub urgency: u8,
    // Close handling needs this flag so history policy stays shared
    pub is_transient: bool,
    // Image metadata intended for UI usage.
    pub image: NotificationImage,
}
