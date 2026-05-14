//! Small normalization helpers for inhibit-related inputs
//!
//! These helpers are pure and easy to unit test in isolation

use unixnotis_core::{INHIBIT_SCOPE_ALL, INHIBIT_SCOPE_POPUPS};

const MAX_INHIBITOR_REASON_BYTES: usize = 256;

pub(super) fn sanitize_inhibit_reason(reason: &str) -> String {
    // Remove leading/trailing space so equivalent inputs produce the same value
    let trimmed = reason.trim();
    if trimmed.is_empty() {
        // Keep an explicit default for empty reasons to avoid blank UI rows
        return "manual".to_string();
    }
    truncate_utf8_bytes(trimmed, MAX_INHIBITOR_REASON_BYTES)
}

pub(super) fn normalize_inhibit_scope(scope: u32) -> zbus::fdo::Result<u32> {
    // Scope "all" is a complete override and can pass through unchanged
    if scope == INHIBIT_SCOPE_ALL {
        return Ok(INHIBIT_SCOPE_ALL);
    }

    // Keep only the flags the daemon actually understands
    let normalized = scope & INHIBIT_SCOPE_POPUPS;
    if normalized == 0 {
        return Err(zbus::fdo::Error::Failed(
            "unsupported inhibit scope".to_string(),
        ));
    }
    Ok(normalized)
}

fn truncate_utf8_bytes(value: &str, max_bytes: usize) -> String {
    if max_bytes == 0 {
        return String::new();
    }
    if value.len() <= max_bytes {
        return value.to_string();
    }

    // Move backward until a valid UTF-8 boundary is found
    let mut end = max_bytes;
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    value[..end].to_string()
}
