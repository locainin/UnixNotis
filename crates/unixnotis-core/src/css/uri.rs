//! Shared URI byte validation for CSS asset references

/// Return whether every percent byte starts a complete hexadecimal escape
#[must_use]
pub fn has_valid_percent_encoding(bytes: &[u8]) -> bool {
    bytes
        .iter()
        .enumerate()
        .filter(|(_index, byte)| **byte == b'%')
        .all(|(index, _byte)| {
            bytes
                .get(index.saturating_add(1)..index.saturating_add(3))
                .is_some_and(|encoded| encoded.iter().all(u8::is_ascii_hexdigit))
        })
}

#[cfg(test)]
#[path = "tests/uri.rs"]
mod tests;
