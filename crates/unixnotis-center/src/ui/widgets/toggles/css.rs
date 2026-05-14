//! Toggle CSS token helpers
//!
//! Keeps CSS class-name normalization rules separate from widget wiring

/// Converts a configured toggle kind into a CSS-safe class suffix
pub(super) fn toggle_kind_css_class(kind: &str) -> Option<String> {
    // GTK CSS class identifiers cannot contain arbitrary punctuation
    // Collapse unknown characters into '-' so output stays deterministic
    let mut out = String::new();
    let mut last_dash = false;

    for ch in kind.chars() {
        // Map to lowercase ASCII plus separators only
        let mapped = match ch {
            'a'..='z' | '0'..='9' => Some(ch),
            'A'..='Z' => Some(ch.to_ascii_lowercase()),
            '-' | '_' => Some('-'),
            _ => Some('-'),
        };
        let ch = mapped?;

        if ch == '-' {
            // Collapse repeated separators to avoid noisy class names
            if last_dash {
                continue;
            }
            last_dash = true;
        } else {
            last_dash = false;
        }

        out.push(ch);
    }

    // Trim leading and trailing separators from noisy inputs
    let token = out.trim_matches('-');
    if token.is_empty() {
        return None;
    }
    // Prefix keeps selector names scoped to toggle cards
    Some(format!("unixnotis-toggle-kind-{token}"))
}

#[cfg(test)]
#[path = "tests/css.rs"]
mod tests;
