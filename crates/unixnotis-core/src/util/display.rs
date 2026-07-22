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

/// Fold an unbroken display token to a bounded column width
#[must_use]
pub fn fold_text_for_layout(value: &str, max_contiguous: usize) -> String {
    if value.is_empty() || max_contiguous == 0 {
        return value.to_string();
    }

    let mut output = String::with_capacity(value.len());
    let mut run_width = 0usize;
    let mut folded_run = false;

    for character in value.chars() {
        if character.is_whitespace() {
            // Whitespace begins a fresh independently bounded token
            run_width = 0;
            folded_run = false;
            output.push(character);
            continue;
        }

        let width = display_width(character);
        if run_width.saturating_add(width) <= max_contiguous {
            output.push(character);
            run_width = run_width.saturating_add(width);
            continue;
        }

        if !folded_run {
            let ellipsis_width = display_width('…');
            // Reclaim only the columns needed for one visible truncation marker
            while run_width.saturating_add(ellipsis_width) > max_contiguous {
                let Some(last) = output.pop() else {
                    break;
                };
                run_width = run_width.saturating_sub(display_width(last));
            }
            if run_width.saturating_add(ellipsis_width) <= max_contiguous {
                output.push('…');
                run_width = run_width.saturating_add(ellipsis_width);
            }
            folded_run = true;
        }
    }

    output
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

fn display_width(character: char) -> usize {
    // Joiners and selectors count as one slot because UI estimators can expose them separately
    if matches!(
        character,
        '\u{200B}' | '\u{200C}' | '\u{200D}' | '\u{2060}' | '\u{FE0E}' | '\u{FE0F}'
    ) {
        return 1;
    }
    UnicodeWidthChar::width_cjk(character).unwrap_or(0)
}

#[cfg(test)]
#[path = "tests/display.rs"]
mod tests;
use unicode_width::UnicodeWidthChar;

pub const MAX_DISPLAY_TOKEN_WIDTH: usize = 96;
