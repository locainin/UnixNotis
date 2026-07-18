//! Bounded terminal and user-interface text sanitizers

/// Sanitizes a log string by stripping newlines and capping length
#[must_use]
pub fn sanitize_log_value(value: &str, max_len: usize) -> String {
    if max_len == 0 {
        return String::new();
    }
    // Pre-allocate to reduce churn when sanitizing frequent log values
    let mut cleaned = String::with_capacity(max_len.min(value.len()));
    let mut count = 0usize;
    let mut truncated = false;
    for ch in value.chars() {
        // Directionality controls can visually reorder terminal output, so drop them
        if is_bidi_control(ch) {
            continue;
        }
        // Replace control/newline bytes with spaces to keep logs single-line and safe
        let ch = if ch.is_control() { ' ' } else { ch };
        cleaned.push(ch);
        count += 1;
        if count >= max_len {
            truncated = true;
            break;
        }
    }
    let trimmed = cleaned.trim();
    if truncated {
        format!("{trimmed}...")
    } else {
        trimmed.to_string()
    }
}

/// Sanitizes text that will be shown to the user inside the UI
#[must_use]
pub fn sanitize_display_text(value: &str) -> String {
    // Multi-line content may preserve newlines after other controls are flattened
    sanitize_display_text_with(value, true)
}

/// Sanitizes multi-line display text while limiting attacker-controlled output
#[must_use]
pub fn sanitize_display_text_bounded(value: &str, max_chars: usize) -> String {
    sanitize_display_text_with_limit(value, true, max_chars)
}

/// Sanitizes text that must remain single-line and safe for display
#[must_use]
pub fn sanitize_inline_display_text(value: &str) -> String {
    // Inline labels stay on one visual line
    sanitize_display_text_with(value, false)
}

fn sanitize_display_text_with(value: &str, keep_newlines: bool) -> String {
    sanitize_display_text_with_limit(value, keep_newlines, usize::MAX)
}

fn sanitize_display_text_with_limit(value: &str, keep_newlines: bool, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }
    // Reserve no more than the output cap even when the source is attacker-controlled
    let mut cleaned = String::with_capacity(value.len().min(max_chars));
    let mut count = 0usize;
    let mut truncated = false;
    for ch in value.chars() {
        // Directionality controls can visually spoof filenames and message text
        if is_bidi_control(ch) {
            continue;
        }

        let mapped = match ch {
            '\n' if keep_newlines => '\n',
            _ if ch.is_control() => ' ',
            _ => ch,
        };
        cleaned.push(mapped);
        count += 1;
        if count >= max_chars {
            truncated = true;
            break;
        }
    }
    if truncated {
        cleaned.push_str("...");
    }
    cleaned
}

const fn is_bidi_control(ch: char) -> bool {
    // Covers directional embeddings, overrides, isolates, and directional marks
    matches!(
        ch,
        '\u{061C}'
            | '\u{200E}'
            | '\u{200F}'
            | '\u{202A}'
            | '\u{202B}'
            | '\u{202C}'
            | '\u{202D}'
            | '\u{202E}'
            | '\u{2066}'
            | '\u{2067}'
            | '\u{2068}'
            | '\u{2069}'
    )
}

#[cfg(test)]
#[path = "tests/display.rs"]
mod tests;
