//! Bounded text operations shared across process and D-Bus boundaries

/// Return an owned prefix no longer than `max_bytes` without splitting a UTF-8 character
#[must_use]
pub fn truncate_utf8_bytes(value: &str, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        // Preserve the complete value when it already fits the caller's byte budget
        return value.to_string();
    }

    // A UTF-8 scalar uses at most four bytes, so only its continuation bytes need inspection
    let mut end = max_bytes;
    while !value.is_char_boundary(end) {
        end -= 1;
    }

    value[..end].to_string()
}

#[cfg(test)]
#[path = "tests/text.rs"]
mod tests;
