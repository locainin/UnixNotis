//! Parsing helpers for slider command output

use unixnotis_core::NumericParseMode;

pub(in super::super) fn parse_numeric(
    text: &str,
    min: f64,
    max: f64,
    mode: NumericParseMode,
) -> Option<f64> {
    // Invalid bounds are rejected here even when a caller bypasses config sanitization
    if !min.is_finite() || !max.is_finite() || min > max {
        return None;
    }

    // Parse the last numeric token and prefer explicit percent tokens
    let mut last_any: Option<(f64, bool, bool)> = None;
    let mut last_percent: Option<(f64, bool)> = None;

    let bytes = text.as_bytes();
    let mut cursor = 0;
    while bytes.get(cursor).is_some() {
        let Some(token) = numeric_token_at(bytes, cursor) else {
            cursor += 1;
            continue;
        };

        // Token bounds always end on ASCII bytes, which are valid UTF-8 boundaries
        if let Ok(value) = text[token.start..token.end].parse::<f64>() {
            let percent = bytes.get(token.end) == Some(&b'%');
            last_any = Some((value, percent, token.has_dot));
            if percent {
                last_percent = Some((value, token.has_dot));
            }
        }
        cursor = token.end;
    }

    let (mut value, percent, has_dot) = if let Some((value, has_dot)) = last_percent {
        // Explicit percent token outranks plain numeric fallback
        (value, true, has_dot)
    } else {
        last_any?
    };

    match mode {
        NumericParseMode::Auto => {
            // Decimal values in small ranges are usually normalized ratios
            // Negative decimals remain ordinary values unless Ratio mode is explicit
            if !percent && has_dot && value.is_sign_positive() && value <= 5.0 {
                value *= 100.0;
            }
        }
        NumericParseMode::Percent => {}
        NumericParseMode::Ratio => {
            if !percent {
                value *= 100.0;
            }
        }
    }

    // Parsing or ratio scaling can overflow even when the source token looked numeric
    if !value.is_finite() {
        return None;
    }

    Some(value.clamp(min, max))
}

#[derive(Clone, Copy)]
struct NumericToken {
    start: usize,
    end: usize,
    has_dot: bool,
}

fn numeric_token_at(bytes: &[u8], start: usize) -> Option<NumericToken> {
    let mut cursor = start;
    if matches!(bytes.get(cursor), Some(b'+' | b'-')) {
        // A sign is unary only at a token boundary, not inside text such as `1-2`
        if !sign_can_start_token(bytes, cursor) {
            return None;
        }
        cursor += 1;
    }

    let integer_start = cursor;
    while bytes.get(cursor).is_some_and(u8::is_ascii_digit) {
        cursor += 1;
    }
    let mut has_digit = cursor > integer_start;
    let mut has_dot = false;

    if bytes.get(cursor) == Some(&b'.') {
        has_dot = true;
        cursor += 1;
        let fraction_start = cursor;
        while bytes.get(cursor).is_some_and(u8::is_ascii_digit) {
            cursor += 1;
        }
        if cursor != fraction_start {
            has_digit = true;
        }
    }

    if !has_digit {
        return None;
    }

    // Keep an exponent only when it contains at least one digit
    if matches!(bytes.get(cursor), Some(b'e' | b'E')) {
        let exponent_mark = cursor;
        cursor += 1;
        if matches!(bytes.get(cursor), Some(b'+' | b'-')) {
            cursor += 1;
        }
        let exponent_start = cursor;
        while bytes.get(cursor).is_some_and(u8::is_ascii_digit) {
            cursor += 1;
        }
        if cursor == exponent_start {
            cursor = exponent_mark;
        }
    }

    Some(NumericToken {
        start,
        end: cursor,
        has_dot,
    })
}

fn sign_can_start_token(bytes: &[u8], start: usize) -> bool {
    let Some(previous) = start.checked_sub(1).and_then(|index| bytes.get(index)) else {
        return true;
    };
    !previous.is_ascii_alphanumeric() && !matches!(previous, b'.' | b'_' | b'+' | b'-')
}

pub(in super::super) fn parse_muted(text: &str) -> bool {
    // Keep checks allocation-free since this runs on every refresh cycle
    contains_ascii_case_insensitive(text, "muted")
        || contains_ascii_case_insensitive(text, "mute: yes")
}

fn contains_ascii_case_insensitive(haystack: &str, needle: &str) -> bool {
    // ASCII byte scan avoids extra allocations and locale-sensitive behavior
    let haystack = haystack.as_bytes();
    let needle = needle.as_bytes();
    if needle.is_empty() {
        return true;
    }
    if haystack.len() < needle.len() {
        return false;
    }
    haystack.windows(needle.len()).any(|window| {
        window
            .iter()
            .zip(needle)
            .all(|(lhs, rhs)| lhs.to_ascii_lowercase() == *rhs)
    })
}

#[cfg(test)]
#[path = "tests/parse.rs"]
mod tests;
