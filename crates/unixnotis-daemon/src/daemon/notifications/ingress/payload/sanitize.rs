//! Bounded hint and action sanitization

use std::collections::HashMap;

use unixnotis_core::util;
use zbus::zvariant::{OwnedValue, Value};

use super::super::limits::{
    MAX_ACTIONS, MAX_ACTION_KEY_BYTES, MAX_ACTION_LABEL_BYTES, MAX_HINT_ENTRIES,
    MAX_HINT_KEY_BYTES, MAX_HINT_STRING_BYTES,
};

pub(in crate::daemon::notifications) fn sanitize_hints_for_storage(
    hints: HashMap<String, OwnedValue>,
) -> HashMap<String, OwnedValue> {
    let mut sanitized = HashMap::with_capacity(hints.len().min(MAX_HINT_ENTRIES));

    for (key, value) in hints {
        // Stop before retaining more hint entries than the model can expose
        if sanitized.len() >= MAX_HINT_ENTRIES {
            break;
        }

        let key = util::truncate_utf8_bytes(key.trim(), MAX_HINT_KEY_BYTES);
        if key.is_empty() {
            continue;
        }

        // Only hints with a defined daemon or presentation meaning survive storage
        let value = match key.as_str() {
            "sound-name" | "sound-file" | "category" => owned_to_string(&value).and_then(|text| {
                let bounded = util::truncate_utf8_bytes(&text, MAX_HINT_STRING_BYTES);
                string_to_owned_value(&bounded)
            }),
            "transient" | "resident" | "suppress-sound" => {
                bool::try_from(&value).ok().map(OwnedValue::from)
            }
            "urgency" => parse_urgency_hint(&value).map(OwnedValue::from),
            _ => None,
        };

        // Unknown values are intentionally dropped instead of being echoed to clients
        if let Some(value) = value {
            sanitized.insert(key, value);
        }
    }

    sanitized
}

pub(in crate::daemon::notifications) fn string_to_owned_value(value: &str) -> Option<OwnedValue> {
    OwnedValue::try_from(Value::from(value)).ok()
}

pub(in crate::daemon::notifications) fn parse_urgency_hint(value: &OwnedValue) -> Option<u32> {
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

pub(in crate::daemon::notifications) fn parse_actions(
    raw: Vec<String>,
) -> Vec<unixnotis_core::Action> {
    // The wire format is a flat key/label sequence, so incomplete pairs are ignored
    let action_capacity = (raw.len() / 2).min(MAX_ACTIONS);
    let mut actions = Vec::with_capacity(action_capacity);
    let mut iter = raw.into_iter();

    while let Some(key) = iter.next() {
        let Some(label) = iter.next() else {
            break;
        };
        // Stop before creating more action state than the UI can render
        if actions.len() >= MAX_ACTIONS {
            break;
        }
        actions.push(unixnotis_core::Action {
            key: util::truncate_utf8_bytes(&key, MAX_ACTION_KEY_BYTES),
            label: util::truncate_utf8_bytes(
                &util::sanitize_inline_display_text(&label),
                MAX_ACTION_LABEL_BYTES,
            ),
        });
    }

    actions
}
