use std::collections::HashSet;

use super::super::model::is_tracked_class;

// Selector checks stay separate so the width parser only deals with declarations
pub(super) fn maybe_warn_for_complex_unixnotis_selector(
    selector: &str,
    has_horizontal_size_rules: bool,
    warnings: &mut Vec<String>,
    warned_classes: &mut HashSet<String>,
) {
    if !has_horizontal_size_rules
        || !is_compound_class_selector(selector)
        || !selector_mentions_tracked_class(selector)
    {
        // Descendant GTK node selectors are noisy here and do not map cleanly to width math
        return;
    }

    let warning_key = format!("complex:{selector}");
    if !warned_classes.insert(warning_key) {
        return;
    }

    warnings.push(format!(
        "size rules target complex UnixNotis selector '{}'; geometry lint only models single-class selectors, so width pressure may be missed",
        selector
    ));
}

pub(super) fn simple_class_selector(selector: &str) -> Option<&str> {
    let trimmed = selector.trim();
    if !trimmed.starts_with('.') {
        // Element names and IDs are outside the small class-based model used here
        return None;
    }
    if trimmed.contains(' ')
        || trimmed.contains('>')
        || trimmed.contains('+')
        || trimmed.contains('~')
        || trimmed.contains(':')
        || trimmed.contains('[')
        || trimmed.contains('#')
        || trimmed.contains(',')
    {
        // Descendant and pseudo selectors are skipped to keep matching conservative
        return None;
    }
    if trimmed.matches('.').count() != 1 {
        // Compound class chains are ambiguous for this lightweight model
        return None;
    }
    Some(trimmed)
}

pub(super) fn is_horizontal_size_property(name: &str) -> bool {
    matches!(
        name.trim(),
        "width"
            | "min-width"
            | "margin"
            | "margin-left"
            | "margin-right"
            | "padding"
            | "padding-left"
            | "padding-right"
            | "border"
            | "border-width"
            | "border-left"
            | "border-left-width"
            | "border-right"
            | "border-right-width"
    )
}

pub(super) fn is_complex_warning_property(name: &str) -> bool {
    // Border-only tweaks are common state styling and usually do not drive row width by themselves
    matches!(
        name.trim(),
        "width"
            | "min-width"
            | "margin"
            | "margin-left"
            | "margin-right"
            | "padding"
            | "padding-left"
            | "padding-right"
    )
}

fn is_compound_class_selector(selector: &str) -> bool {
    let trimmed = selector.trim();
    // Only same-element class chains are warned here
    trimmed.starts_with('.')
        && trimmed.matches('.').count() > 1
        && !trimmed.contains(' ')
        && !trimmed.contains('>')
        && !trimmed.contains('+')
        && !trimmed.contains('~')
        && !trimmed.contains(':')
        && !trimmed.contains('[')
        && !trimmed.contains('#')
        && !trimmed.contains(',')
}

fn selector_mentions_tracked_class(selector: &str) -> bool {
    let bytes = selector.as_bytes();
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index] != b'.' {
            index += 1;
            continue;
        }

        let start = index;
        index += 1;
        while index < bytes.len() {
            let byte = bytes[index];
            // Class scanning stops at the first non class-name byte
            if byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_' {
                index += 1;
            } else {
                break;
            }
        }

        if index > start + 1 && is_tracked_class(&selector[start..index]) {
            // One tracked class is enough to make the selector relevant to geometry lint
            return true;
        }
    }

    false
}
