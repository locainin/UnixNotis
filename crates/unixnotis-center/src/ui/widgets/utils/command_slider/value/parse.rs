//! Parsing helpers for slider command output

use unixnotis_core::NumericParseMode;

pub(in super::super) fn parse_numeric(
    text: &str,
    min: f64,
    max: f64,
    mode: NumericParseMode,
) -> Option<f64> {
    // Parse the last numeric token and prefer explicit percent tokens
    let mut current_start = None;
    let mut current_has_dot = false;
    let mut last_any: Option<(f64, bool, bool)> = None;
    let mut last_percent: Option<(f64, bool)> = None;

    for (index, ch) in text.char_indices() {
        if ch.is_ascii_digit() || ch == '.' {
            if current_start.is_none() {
                current_start = Some(index);
            }
            if ch == '.' {
                current_has_dot = true;
            }
            continue;
        }
        if let Some(start) = current_start.take() {
            // Token boundary reached, parse collected numeric segment
            if let Ok(value) = text[start..index].parse::<f64>() {
                let percent = ch == '%';
                last_any = Some((value, percent, current_has_dot));
                if percent {
                    last_percent = Some((value, current_has_dot));
                }
            }
            current_has_dot = false;
        }
    }

    if let Some(start) = current_start.take() {
        if let Ok(value) = text[start..].parse::<f64>() {
            last_any = Some((value, false, current_has_dot));
        }
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
            if !percent && has_dot && value <= 5.0 {
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

    Some(value.clamp(min, max))
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
