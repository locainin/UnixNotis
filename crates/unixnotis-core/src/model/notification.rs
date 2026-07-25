//! Notification records and their lightweight UI views

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use zbus::zvariant::{OwnedValue, Type};

use super::attribution::NotificationAttribution;
use super::image::NotificationImage;
use super::reply::InlineReply;
use super::types::{Action, Urgency};
use crate::util::{fold_text_for_layout, MAX_DISPLAY_TOKEN_WIDTH};

/// Full notification record stored by the daemon
#[derive(Debug)]
pub struct Notification {
    // Stable identifier assigned by the daemon
    pub id: u32,
    // Origin metadata for display and filtering
    pub app_name: String,
    pub app_icon: String,
    // User-facing content as provided by the sender
    pub summary: String,
    pub body: String,
    // Optional actions supplied by the app
    pub actions: Vec<Action>,
    // Reply metadata exists only for an explicit KDE-compatible action
    pub inline_reply: InlineReply,
    // Raw hints preserved for storage and downstream consumers
    pub hints: HashMap<String, OwnedValue>,
    // Derived urgency used for styling and escalation
    pub urgency: Urgency,
    pub category: Option<String>,
    // Flags from the notification protocol
    pub is_transient: bool,
    pub is_resident: bool,
    /// Suppress showing this notification as a popup
    pub suppress_popup: bool,
    /// Suppress sound playback for this notification
    pub suppress_sound: bool,
    pub image: NotificationImage,
    pub expire_timeout: i32,
    pub received_at: DateTime<Utc>,
    // D-Bus unique sender name for ownership checks in daemon-side operations
    pub sender_name: Option<String>,
    // Sender process metadata is retained for diagnostics and audit logging
    pub sender_pid: Option<u32>,
    pub sender_start_time: Option<u64>,
    pub sender_executable: Option<String>,
}

impl Notification {
    /// Convert to a lightweight view for UI consumption
    #[must_use]
    pub fn to_view(&self) -> NotificationView {
        let (app_name, attribution) =
            NotificationAttribution::resolve(&self.app_name, self.sender_executable.as_deref());
        NotificationView {
            id: self.id,
            app_name,
            attribution,
            summary: notification_display_text(&self.summary),
            body: notification_display_text(&self.body),
            actions: self.actions.clone(),
            inline_reply: self.inline_reply.clone(),
            urgency: self.urgency.as_u8(),
            // Center and popup policy both need the transient bit to stay in sync
            is_transient: self.is_transient,
            // UIs only need the text, actions, and image payload used for rendering
            image: self.image.clone(),
            // Protocol flags and sender metadata stay daemon-side to keep D-Bus payloads small
        }
    }

    /// Convert to a view for list rows with heavy image data removed
    #[must_use]
    pub fn to_list_view(&self) -> NotificationView {
        let (app_name, attribution) =
            NotificationAttribution::resolve(&self.app_name, self.sender_executable.as_deref());
        NotificationView {
            id: self.id,
            app_name,
            attribution,
            summary: notification_display_text(&self.summary),
            body: notification_display_text(&self.body),
            actions: self.actions.clone(),
            inline_reply: self.inline_reply.clone(),
            urgency: self.urgency.as_u8(),
            // History policy still depends on the transient bit in panel rows
            is_transient: self.is_transient,
            // List rows should avoid carrying raw image buffers across D-Bus
            image: self.image.for_listing(),
            // Protocol flags and sender metadata stay daemon-side to keep D-Bus payloads small
        }
    }

    /// Create a history entry with heavyweight hint data stripped out
    #[must_use]
    pub fn to_history(&self) -> Self {
        // History entries should never retain raw image-data blobs
        let mut image = self.image.clone();
        image.has_image_data = false;
        image.image_data = Default::default();
        Self {
            id: self.id,
            app_name: self.app_name.clone(),
            app_icon: self.app_icon.clone(),
            summary: self.summary.clone(),
            body: self.body.clone(),
            actions: self.actions.clone(),
            inline_reply: self.inline_reply.clone(),
            // Keep history entries lightweight by dropping raw hint payloads
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

fn notification_plain_text(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '<' => {
                // Notification bodies may contain simple HTML-like tags from desktop senders
                let mut tag = String::new();
                let mut closed = false;
                for next in chars.by_ref() {
                    if next == '>' {
                        // A closed tag is formatting, not text that belongs in a GTK label
                        closed = true;
                        break;
                    }
                    tag.push(next);
                }
                if closed {
                    // Block-like tags get spacing so joined text still reads naturally
                    push_tag_spacing(&mut output, &tag);
                } else {
                    // Broken markup stays visible so the sender's text is not silently lost
                    output.push('<');
                    output.push_str(&tag);
                }
            }
            // Entities are decoded here because GTK labels receive plain text
            '&' => output.push_str(&decode_entity(&mut chars)),
            _ => output.push(ch),
        }
    }

    // Tag stripping can leave noisy gaps, so normalize once at the end
    collapse_notification_whitespace(&output)
}

fn notification_display_text(input: &str) -> String {
    // Markup removal can join text that was separated by tags in the stored payload
    fold_text_for_layout(&notification_plain_text(input), MAX_DISPLAY_TOKEN_WIDTH)
}

fn push_tag_spacing(output: &mut String, tag: &str) {
    const BLOCK_TAGS: [&str; 5] = ["br", "p", "div", "li", "tr"];

    // Trim "/" first so opening and closing tags use the same spacing rule
    let tag_name = tag
        .trim_start_matches('/')
        .split(|ch: char| ch.is_whitespace() || ch == '/')
        .next()
        .unwrap_or_default();
    if BLOCK_TAGS
        .iter()
        .any(|expected| tag_name.eq_ignore_ascii_case(expected))
    {
        // These tags normally separate chunks of text
        output.push('\n');
    }
}

fn decode_entity<I>(chars: &mut std::iter::Peekable<I>) -> String
where
    I: Iterator<Item = char>,
{
    let mut entity = String::new();
    let mut terminated = false;
    while let Some(&next) = chars.peek() {
        chars.next();
        if next == ';' {
            // Only a semicolon-ended entity should be decoded
            terminated = true;
            break;
        }
        if entity.len() >= 16 {
            // Very long entities are likely plain text or malformed sender data
            return format!("&{entity}{next}");
        }
        entity.push(next);
    }

    if !terminated {
        // Unterminated entities are kept literal, including common names like &amp
        return format!("&{entity}");
    }

    match entity.as_str() {
        // Keep the named set intentionally small and predictable
        "amp" => "&".to_string(),
        "apos" => "'".to_string(),
        "gt" => ">".to_string(),
        "lt" => "<".to_string(),
        "nbsp" => " ".to_string(),
        "quot" => "\"".to_string(),
        _ => decode_numeric_entity(&entity).unwrap_or_else(|| format!("&{entity};")),
    }
}

fn decode_numeric_entity(entity: &str) -> Option<String> {
    // Desktop notifications can send both decimal and hex numeric entities
    let value = if let Some(hex) = entity
        .strip_prefix("#x")
        .or_else(|| entity.strip_prefix("#X"))
    {
        u32::from_str_radix(hex, 16).ok()?
    } else {
        entity.strip_prefix('#')?.parse::<u32>().ok()?
    };
    // Invalid scalar values are not text, so the caller keeps the original entity
    char::from_u32(value).map(|ch| ch.to_string())
}

fn collapse_notification_whitespace(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut saw_space = false;
    let mut saw_newline = false;

    for ch in input.chars() {
        if ch == '\n' {
            // Keep one newline for block boundaries, but avoid tall empty gaps
            if !output.is_empty() && !saw_newline {
                output.push('\n');
            }
            saw_space = false;
            saw_newline = true;
        } else if ch.is_whitespace() {
            // Plain spaces collapse to one space unless a newline already separated text
            if !output.is_empty() && !saw_space && !saw_newline {
                output.push(' ');
            }
            saw_space = true;
        } else {
            // Any real character resets spacing guards
            output.push(ch);
            saw_space = false;
            saw_newline = false;
        }
    }

    if saw_newline {
        // A newline can follow an already-normalized space at the tail
        output.pop();
        if output.ends_with(' ') {
            output.pop();
        }
    } else if saw_space {
        output.pop();
    }

    output
}

/// Serializable view of a notification for D-Bus signals
#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq)]
pub struct NotificationView {
    // Identifier matches Notification::id
    pub id: u32,
    // Lightweight fields used for UI display and filtering
    // Intentionally omits daemon-only protocol flags and timestamps
    pub app_name: String,
    // Authenticated badge identity and any mismatched caller-supplied brand claim
    pub attribution: NotificationAttribution,
    pub summary: String,
    pub body: String,
    pub actions: Vec<Action>,
    pub inline_reply: InlineReply,
    pub urgency: u8,
    // Close handling needs this flag so history policy stays shared
    pub is_transient: bool,
    // Image metadata intended for UI usage
    pub image: NotificationImage,
}

impl NotificationView {
    /// Visible primary and secondary attribution for UI and diagnostic surfaces
    #[must_use]
    pub fn attribution_label(&self) -> String {
        self.attribution.display_label(&self.app_name)
    }
}

#[cfg(test)]
#[path = "tests/notification.rs"]
mod tests;
