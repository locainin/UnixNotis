//! Payload construction and sanitization for notifications
//!
//! This module turns raw D-Bus values into bounded internal model values

use std::cmp::Ordering;
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use unixnotis_core::{
    decode_image_asset_contents, util, Action, AssetPolicy, AttributionDiagnostics, Config,
    ImageData, InlineReply, InlineReplyPolicy, Notification, NotificationAttribution,
    NotificationImage, Urgency, DEFAULT_ICON_ASSET_EXTENSIONS, DEFAULT_ICON_ASSET_MAX_HEIGHT,
    DEFAULT_ICON_ASSET_MAX_PIXELS, DEFAULT_ICON_ASSET_MAX_WIDTH,
};
use zbus::zvariant::{OwnedValue, Value};

use super::super::identity::SenderMetadata;
use super::limits::{
    MAX_ACTIONS, MAX_ACTION_KEY_BYTES, MAX_ACTION_LABEL_BYTES, MAX_APP_ICON_BYTES,
    MAX_APP_NAME_BYTES, MAX_BODY_BYTES, MAX_CATEGORY_BYTES, MAX_HINT_ENTRIES, MAX_HINT_KEY_BYTES,
    MAX_HINT_STRING_BYTES, MAX_SUMMARY_BYTES,
};

pub(in crate::daemon::notifications) struct NotificationInput {
    pub(in crate::daemon::notifications) app_name: String,
    pub(in crate::daemon::notifications) app_icon: String,
    pub(in crate::daemon::notifications) summary: String,
    pub(in crate::daemon::notifications) body: String,
    pub(in crate::daemon::notifications) actions: Vec<String>,
    pub(in crate::daemon::notifications) hints: HashMap<String, OwnedValue>,
    pub(in crate::daemon::notifications) image_data: Option<ImageData>,
    pub(in crate::daemon::notifications) conversation_avatar: Option<ImageData>,
    pub(in crate::daemon::notifications) sender: SenderMetadata,
    pub(in crate::daemon::notifications) attribution: NotificationAttribution,
    pub(in crate::daemon::notifications) attribution_diagnostics: AttributionDiagnostics,
    pub(in crate::daemon::notifications) inline_reply_policy: InlineReplyPolicy,
    pub(in crate::daemon::notifications) expire_timeout: i32,
}

pub(in crate::daemon::notifications) fn build_notification(
    input: NotificationInput,
) -> Notification {
    let NotificationInput {
        app_name,
        app_icon,
        summary,
        body,
        actions,
        hints,
        image_data,
        conversation_avatar,
        sender,
        attribution,
        attribution_diagnostics,
        inline_reply_policy,
        expire_timeout,
    } = input;

    // Read shared hint data first
    let urgency = Urgency::from_hint(hints.get("urgency"));
    let category = hints
        .get("category")
        .and_then(owned_to_string)
        .map(|value| {
            // Category stays on one line
            util::truncate_utf8_bytes(
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
    let mut image = NotificationImage::from_hints(&app_name, &app_icon, &hints);
    if let Some(image_data) = image_data {
        // The wire decoder already normalized this bounded image without dynamic byte expansion
        image.has_image_data = true;
        image.image_data = image_data;
    }
    // Only positive application association may expose a decoded sender avatar
    if matches!(
        attribution.status,
        unixnotis_core::AttributionStatus::Verified | unixnotis_core::AttributionStatus::Recognized
    ) {
        if let Some(avatar) = conversation_avatar {
            // The avatar is already decoded and bounded before this model is stored
            image.has_conversation_avatar = true;
            image.conversation_avatar = avatar;
        }
    }

    // Only verified senders may name host files for decoding. Untrusted,
    // conflicting, relay, and portal-associated senders are stripped of
    // host file paths to prevent parser delegation attacks (UNX-4-003).
    if !attribution.is_verified() {
        image.image_path = String::new();
    }
    let actions = parse_actions(actions);
    // Protocol metadata is parsed independently from the daemon's interaction decision
    let inline_reply = parse_inline_reply(&actions, &hints);
    // Clean text before storing it
    let app_name = util::sanitize_inline_display_text(&app_name);
    let summary = util::sanitize_display_text(&summary);
    let body = util::sanitize_display_text(&body);

    Notification {
        id: 0,
        // The store assigns a process-wide generation during the commit
        generation: 0,
        app_name: if app_name.is_empty() {
            // Keep explicit fallback text for empty callers
            "Unknown".to_string()
        } else {
            util::truncate_utf8_bytes(&app_name, MAX_APP_NAME_BYTES)
        },
        app_icon: util::truncate_utf8_bytes(&app_icon, MAX_APP_ICON_BYTES),
        attribution,
        attribution_diagnostics,
        // Truncate bytes first, then fold long contiguous runs to keep UTF-8 boundaries valid
        // Fold very long unbroken runs so renderer width remains bounded
        summary: util::fold_text_for_layout(
            &util::truncate_utf8_bytes(&summary, MAX_SUMMARY_BYTES),
            util::MAX_DISPLAY_TOKEN_WIDTH,
        ),
        // Apply the same order for body so renderer sees consistent text constraints
        // Body can be much larger, so apply the same run-folding protection here
        body: util::fold_text_for_layout(
            &util::truncate_utf8_bytes(&body, MAX_BODY_BYTES),
            util::MAX_DISPLAY_TOKEN_WIDTH,
        ),
        actions,
        inline_reply,
        inline_reply_policy,
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

// Keep sender-provided avatar work separate from the normal application badge path
const MAX_CONVERSATION_AVATAR_BYTES: u64 = 2 * 1024 * 1024;
pub(in crate::daemon::notifications) const CONVERSATION_AVATAR_TIMEOUT: Duration =
    Duration::from_millis(500);

pub(in crate::daemon::notifications) fn communication_notification_candidate(
    hints: &HashMap<String, OwnedValue>,
    actions: &[String],
) -> bool {
    // Inline reply is a stronger communication signal than a caller label
    if actions
        .chunks_exact(2)
        .any(|pair| pair.first().is_some_and(|key| key == "inline-reply"))
    {
        return true;
    }
    // Categories are protocol metadata and remain only a presentation hint
    hints
        .get("category")
        .and_then(owned_to_string)
        .is_some_and(|category| {
            let category = category.to_ascii_lowercase();
            ["im", "chat", "message", "email", "mail"]
                .iter()
                .any(|marker| category.split('.').any(|part| part == *marker))
        })
}

pub(in crate::daemon::notifications) fn materialize_conversation_avatar(
    app_icon: &str,
) -> Option<ImageData> {
    // Decode while the daemon still controls the file read and parser limits
    let path = local_avatar_path(app_icon)?;
    let mut file = File::open(&path).ok()?;
    let metadata = file.metadata().ok()?;
    if !metadata.is_file() {
        return None;
    }
    if !avatar_file_size_allowed(metadata.len()) {
        return None;
    }
    let mut bytes = Vec::new();
    file.by_ref()
        .take(MAX_CONVERSATION_AVATAR_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)
        .ok()?;
    if !avatar_buffer_size_allowed(bytes.len()) {
        return None;
    }
    // The small policy keeps contact art from becoming an unbounded texture
    let policy = AssetPolicy {
        max_bytes: MAX_CONVERSATION_AVATAR_BYTES,
        max_width: DEFAULT_ICON_ASSET_MAX_WIDTH.min(256),
        max_height: DEFAULT_ICON_ASSET_MAX_HEIGHT.min(256),
        max_pixels: DEFAULT_ICON_ASSET_MAX_PIXELS.min(65_536),
        allowed_extensions: DEFAULT_ICON_ASSET_EXTENSIONS,
    };
    let decoded = decode_image_asset_contents(&path, &bytes, policy).ok()?;
    let width = i32::try_from(decoded.width).ok()?;
    let height = i32::try_from(decoded.height).ok()?;
    let rowstride = width.checked_mul(4)?;
    let expected = usize::try_from(rowstride)
        .ok()?
        .checked_mul(usize::try_from(height).ok()?)?;
    if decoded.rgba.len() != expected {
        return None;
    }
    Some(ImageData {
        width,
        height,
        rowstride,
        has_alpha: true,
        bits_per_sample: 8,
        channels: 4,
        data: decoded.rgba,
    })
}

const fn avatar_file_size_allowed(size: u64) -> bool {
    size <= MAX_CONVERSATION_AVATAR_BYTES
}

const fn avatar_buffer_size_allowed(size: usize) -> bool {
    size <= MAX_CONVERSATION_AVATAR_BYTES as usize
}

fn local_avatar_path(value: &str) -> Option<PathBuf> {
    if value.starts_with('/') {
        return Some(PathBuf::from(value));
    }
    let path = value.strip_prefix("file://")?;
    let path = path.strip_prefix("localhost/").unwrap_or(path);
    path.starts_with('/').then(|| Path::new(path).to_path_buf())
}

pub(in crate::daemon::notifications) fn resolve_expiration(
    config: &Config,
    notification: &Notification,
) -> Option<Instant> {
    // Resident notifications never auto-expire
    if notification.is_resident {
        return None;
    }

    let timeout_ms = match notification.expire_timeout.cmp(&0) {
        // Explicit timeout=0 disables auto-expiration
        Ordering::Equal => return None,
        // Positive values are caller-provided milliseconds
        Ordering::Greater => notification.expire_timeout as u64,
        // Negative values request defaults by urgency
        Ordering::Less => match notification.urgency {
            Urgency::Critical => config.popups.critical_timeout_ms?,
            _ => config.popups.default_timeout_ms,
        },
    };

    if timeout_ms == 0 {
        return None;
    }

    Some(Instant::now() + Duration::from_millis(timeout_ms))
}

fn parse_inline_reply(actions: &[Action], hints: &HashMap<String, OwnedValue>) -> InlineReply {
    let Some(action) = actions.iter().find(|action| action.key == "inline-reply") else {
        // Reply hints without the protocol action cannot create a text control
        return InlineReply::default();
    };

    InlineReply {
        available: true,
        label: action.label.clone(),
        placeholder: reply_hint_text(hints, "x-kde-reply-placeholder-text"),
        submit_label: reply_hint_text(hints, "x-kde-reply-submit-button-text"),
        submit_icon: reply_hint_text(hints, "x-kde-reply-submit-button-icon-name"),
    }
}

fn reply_hint_text(hints: &HashMap<String, OwnedValue>, key: &str) -> String {
    let Some(value) = hints.get(key).and_then(owned_to_string) else {
        return String::new();
    };
    // Reply controls are single-line GTK widgets, so layout controls are removed here
    let clean = util::sanitize_inline_display_text(&value);
    util::truncate_utf8_bytes(&clean, MAX_HINT_STRING_BYTES)
}

fn parse_actions(raw: Vec<String>) -> Vec<Action> {
    // Actions come in key and label pairs
    let action_capacity = (raw.len() / 2).min(MAX_ACTIONS);
    let mut actions = Vec::with_capacity(action_capacity);
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
                key: util::truncate_utf8_bytes(&key, MAX_ACTION_KEY_BYTES),
                // Label is shown to the user
                label: util::truncate_utf8_bytes(
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

        let key = util::truncate_utf8_bytes(key.trim(), MAX_HINT_KEY_BYTES);
        if key.is_empty() {
            continue;
        }

        let value = match key.as_str() {
            // Keep only hints that matter for daemon behavior and rendering
            "sound-name" | "sound-file" | "category" => owned_to_string(&value).and_then(|text| {
                // Keep hint text small
                let bounded = util::truncate_utf8_bytes(&text, MAX_HINT_STRING_BYTES);
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
        return Some(u32::from(raw).min(2));
    }
    if let Ok(raw) = u32::try_from(value) {
        return Some(raw.min(2));
    }
    None
}

pub(in crate::daemon::notifications) fn owned_to_string(value: &OwnedValue) -> Option<String> {
    value
        .try_clone()
        .ok()
        .and_then(|owned| String::try_from(owned).ok())
}

#[cfg(test)]
#[path = "tests/payload.rs"]
mod tests;
