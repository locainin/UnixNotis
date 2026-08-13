//! Bounded text operations shared across process and D-Bus boundaries

/// Return an owned prefix no longer than `max_bytes` without splitting a UTF-8 character
#[must_use]
pub fn truncate_utf8_bytes(value: &str, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        // Preserve the complete value when it already fits the caller's byte budget
        return value.to_string();
    }

    // A UTF-8 scalar uses at most four bytes, so this range examines no more than four offsets
    let end = (max_bytes.saturating_sub(3)..=max_bytes)
        .rev()
        .find(|offset| value.is_char_boundary(*offset))
        .unwrap_or_default();

    value[..end].to_string()
}

#[cfg(test)]
#[path = "tests/text.rs"]
mod tests;
