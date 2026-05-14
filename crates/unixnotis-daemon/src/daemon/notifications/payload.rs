//! Payload construction and sanitization for notifications
//!
//! This module turns raw D-Bus values into bounded internal model values

use std::collections::HashMap;
use std::time::{Duration, Instant};

use unicode_width::UnicodeWidthChar;
use unixnotis_core::{util, Action, Config, Notification, NotificationImage, Urgency};
use zbus::zvariant::{OwnedValue, Value};

use super::limits::{
    MAX_ACTIONS, MAX_ACTION_KEY_BYTES, MAX_ACTION_LABEL_BYTES, MAX_APP_ICON_BYTES,
    MAX_APP_NAME_BYTES, MAX_BODY_BYTES, MAX_CATEGORY_BYTES, MAX_HINT_ENTRIES, MAX_HINT_KEY_BYTES,
    MAX_HINT_STRING_BYTES, MAX_SUMMARY_BYTES,
};
use super::sender::SenderMetadata;

// Unbroken tokens longer than this are folded with an ellipsis to avoid UI overflow spikes.
const MAX_CONTIGUOUS_TOKEN_CHARS: usize = 96;

pub(super) struct NotificationInput {
    pub(super) app_name: String,
    pub(super) app_icon: String,
    pub(super) summary: String,
    pub(super) body: String,
    pub(super) actions: Vec<String>,
    pub(super) hints: HashMap<String, OwnedValue>,
    pub(super) sender: SenderMetadata,
    pub(super) expire_timeout: i32,
}

pub(super) fn build_notification(input: NotificationInput) -> Notification {
    let NotificationInput {
        app_name,
        app_icon,
        summary,
        body,
        actions,
        hints,
        sender,
        expire_timeout,
    } = input;

    // Read shared hint data first
    let urgency = Urgency::from_hint(hints.get("urgency"));
    let category = hints
        .get("category")
        .and_then(owned_to_string)
        .map(|value| {
            // Category stays on one line
            truncate_utf8_bytes(
                &util::sanitize_inline_display_text(&value),
                MAX_CATEGORY_BYTES,
            )
        });
    let is_transient = hints
        .get("transient")
        .and_then(|value| bool::try_from(value).ok())
        .unwrap_or(false);
    let is_resident = hints
        .get("resident")
        .and_then(|value| bool::try_from(value).ok())
        .unwrap_or(false);
    let image = NotificationImage::from_hints(&app_name, &app_icon, &hints);
    // Clean text before storing it
    let app_name = util::sanitize_inline_display_text(&app_name);
    let summary = util::sanitize_display_text(&summary);
    let body = util::sanitize_display_text(&body);

    Notification {
        id: 0,
        app_name: if app_name.is_empty() {
            // Keep explicit fallback text for empty callers
            "Unknown".to_string()
        } else {
            truncate_utf8_bytes(&app_name, MAX_APP_NAME_BYTES)
        },
        app_icon: truncate_utf8_bytes(&app_icon, MAX_APP_ICON_BYTES),
        // Truncate bytes first, then fold long contiguous runs to keep UTF-8 boundaries valid
        // Fold very long unbroken runs so renderer width remains bounded.
        summary: normalize_text_for_layout(
            &truncate_utf8_bytes(&summary, MAX_SUMMARY_BYTES),
            MAX_CONTIGUOUS_TOKEN_CHARS,
        ),
        // Apply the same order for body so renderer sees consistent text constraints
        // Body can be much larger, so apply the same run-folding protection here.
        body: normalize_text_for_layout(
            &truncate_utf8_bytes(&body, MAX_BODY_BYTES),
            MAX_CONTIGUOUS_TOKEN_CHARS,
        ),
        actions: parse_actions(actions),
        // Keep only needed hints
        hints: sanitize_hints_for_storage(hints),
        urgency,
        category,
        is_transient,
        is_resident,
        suppress_popup: false,
        suppress_sound: false,
        image,
        expire_timeout,
        received_at: chrono::Utc::now(),
        sender_name: sender.sender_name,
        sender_pid: sender.sender_pid,
        sender_start_time: sender.sender_start_time,
        sender_executable: sender.sender_executable,
    }
}

pub(super) fn resolve_expiration(config: &Config, notification: &Notification) -> Option<Instant> {
    // Explicit timeout=0 and resident notifications never auto-expire
    if notification.expire_timeout == 0 || notification.is_resident {
        return None;
    }

    // Positive values are caller-provided milliseconds
    let timeout_ms = if notification.expire_timeout > 0 {
        notification.expire_timeout as u64
    } else {
        // Negative values request defaults by urgency
        match notification.urgency {
            Urgency::Critical => config.popups.critical_timeout_ms?,
            _ => config.popups.default_timeout_ms,
        }
    };

    if timeout_ms == 0 {
        return None;
    }

    Some(Instant::now() + Duration::from_millis(timeout_ms))
}

fn parse_actions(raw: Vec<String>) -> Vec<Action> {
    // Actions come in key and label pairs
    let mut actions = Vec::with_capacity(raw.len().min(MAX_ACTIONS));
    let mut iter = raw.into_iter();

    // The protocol sends actions as [key, label, key, label, ...]
    while let Some(key) = iter.next() {
        if let Some(label) = iter.next() {
            if actions.len() >= MAX_ACTIONS {
                // Hard stop keeps button rows bounded even when sender floods action pairs
                break;
            }
            actions.push(Action {
                // Key is protocol data
                key: truncate_utf8_bytes(&key, MAX_ACTION_KEY_BYTES),
                // Label is shown to the user
                label: truncate_utf8_bytes(
                    &util::sanitize_inline_display_text(&label),
                    MAX_ACTION_LABEL_BYTES,
                ),
            });
        }
    }
    actions
}

fn sanitize_hints_for_storage(hints: HashMap<String, OwnedValue>) -> HashMap<String, OwnedValue> {
    // Pre-sizing avoids rehash churn on adversarial hint fanout
    let mut sanitized = HashMap::with_capacity(hints.len().min(MAX_HINT_ENTRIES));

    for (key, value) in hints {
        if sanitized.len() >= MAX_HINT_ENTRIES {
            break;
        }

        let key = truncate_utf8_bytes(key.trim(), MAX_HINT_KEY_BYTES);
        if key.is_empty() {
            continue;
        }

        let value = match key.as_str() {
            // Keep only hints that matter for daemon behavior and rendering
            "sound-name" | "sound-file" | "category" => owned_to_string(&value).and_then(|text| {
                // Keep hint text small
                let bounded = truncate_utf8_bytes(&text, MAX_HINT_STRING_BYTES);
                string_to_owned_value(&bounded)
            }),
            "transient" | "resident" | "suppress-sound" => {
                bool::try_from(&value).ok().map(OwnedValue::from)
            }
            "urgency" => parse_urgency_hint(&value).map(OwnedValue::from),
            _ => None,
        };

        if let Some(value) = value {
            sanitized.insert(key, value);
        }
    }

    sanitized
}

fn string_to_owned_value(value: &str) -> Option<OwnedValue> {
    OwnedValue::try_from(Value::from(value)).ok()
}

fn parse_urgency_hint(value: &OwnedValue) -> Option<u32> {
    // Accept both byte and integer variants from mixed clients
    if let Ok(raw) = u8::try_from(value) {
        return Some((raw as u32).min(2));
    }
    if let Ok(raw) = u32::try_from(value) {
        return Some(raw.min(2));
    }
    None
}

fn owned_to_string(value: &OwnedValue) -> Option<String> {
    value
        .try_clone()
        .ok()
        .and_then(|owned| String::try_from(owned).ok())
}

fn truncate_utf8_bytes(value: &str, max_bytes: usize) -> String {
    if max_bytes == 0 {
        return String::new();
    }
    if value.len() <= max_bytes {
        // Fast path for common short payloads
        return value.to_string();
    }

    // Move backward until UTF-8 stays valid
    let mut end = max_bytes;
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    value[..end].to_string()
}

fn normalize_text_for_layout(value: &str, max_contiguous: usize) -> String {
    if value.is_empty() || max_contiguous == 0 {
        return value.to_string();
    }

    // Reserve original length to keep this pass allocation-stable on long input
    let mut out = String::with_capacity(value.len());
    let mut run_width = 0usize;
    let mut folded_run = false;

    // Walk characters so non-ASCII content remains valid after normalization.
    // Width is tracked in display columns instead of char count to handle wide glyphs.
    for ch in value.chars() {
        if ch.is_whitespace() {
            // Whitespace resets contiguous-run accounting
            run_width = 0;
            folded_run = false;
            out.push(ch);
            continue;
        }

        let width = display_width(ch);
        if run_width.saturating_add(width) <= max_contiguous {
            // Short runs stay as they are
            out.push(ch);
            run_width = run_width.saturating_add(width);
            continue;
        }

        // Add one ellipsis when a contiguous token crosses the safety threshold.
        if !folded_run {
            let ellipsis_width = display_width('…');
            // Keep final run width bounded by trimming the current run tail first.
            while run_width.saturating_add(ellipsis_width) > max_contiguous {
                // Pop one char at a time
                let Some(last) = out.pop() else {
                    break;
                };
                run_width = run_width.saturating_sub(display_width(last));
            }
            if run_width.saturating_add(ellipsis_width) <= max_contiguous {
                out.push('…');
                run_width = run_width.saturating_add(ellipsis_width);
            }
            folded_run = true;
        }
        // Remaining chars in this run are dropped until whitespace appears again
    }

    out
}

fn display_width(ch: char) -> usize {
    // Width estimators in downstream UI surfaces often treat joiners/selectors as visible slots.
    // Counting them here keeps folded output safely within those stricter layouts.
    if matches!(
        ch,
        '\u{200B}' | '\u{200C}' | '\u{200D}' | '\u{2060}' | '\u{FE0E}' | '\u{FE0F}'
    ) {
        return 1;
    }
    UnicodeWidthChar::width_cjk(ch).unwrap_or(0)
}

#[cfg(test)]
#[path = "payload_tests.rs"]
mod tests;
