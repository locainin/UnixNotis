//! Notification records and their lightweight UI views.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use zbus::zvariant::{OwnedValue, Type};

use super::model_image::NotificationImage;
use super::model_types::{Action, Urgency};

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
            is_transient: self.is_transient,
            is_resident: self.is_resident,
            received_at_unix_ms: self.received_at.timestamp_millis(),
            image: self.image.clone(),
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
            is_transient: self.is_transient,
            is_resident: self.is_resident,
            received_at_unix_ms: self.received_at.timestamp_millis(),
            image: self.image.for_listing(),
        }
    }

    /// Create a history entry with heavyweight hint data stripped out.
    pub fn to_history(&self) -> Notification {
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
            image: self.image.for_history(),
            expire_timeout: self.expire_timeout,
            received_at: self.received_at,
        }
    }
}

/// Serializable view of a notification for D-Bus signals.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct NotificationView {
    // Identifier matches Notification::id.
    pub id: u32,
    // Lightweight fields used for UI display and filtering.
    pub app_name: String,
    pub summary: String,
    pub body: String,
    pub actions: Vec<Action>,
    pub urgency: u8,
    pub is_transient: bool,
    pub is_resident: bool,
    // Epoch milliseconds for stable sorting and relative time formatting.
    pub received_at_unix_ms: i64,
    // Image metadata intended for UI usage.
    pub image: NotificationImage,
}
