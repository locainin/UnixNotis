use std::collections::HashSet;

use super::super::stock::baselines::stock_matches_complex_selector_rules;

// Selector checks stay separate so the width parser only deals with declarations
pub(super) fn maybe_warn_for_complex_unixnotis_selector(
    selector: &str,
    properties: &[(String, String)],
    has_horizontal_size_rules: bool,
    warnings: &mut Vec<String>,
    warned_classes: &mut HashSet<String>,
) {
    if !has_horizontal_size_rules || !selector_mentions_unixnotis_class(selector) {
        // Cosmetic selectors and non-UnixNotis selectors are noise here
        return;
    }
    // Anything outside the plain single-class path is too loose for the small width model
    if simple_class_selector(selector).is_some() {
        // Plain single-class selectors already go through the normal width model path
        return;
    }
    if stock_matches_complex_selector_rules(selector, properties) {
        // Shipped complex selectors already have a known baseline and should stay quiet
        return;
    }

    let warning_key = format!("complex:{selector}");
    if !warned_classes.insert(warning_key) {
        // One warning per selector is enough even if the selector appears more than once
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

fn selector_mentions_unixnotis_class(selector: &str) -> bool {
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

        if index > start + 1 && selector[start..index].starts_with(".unixnotis-") {
            // One UnixNotis class is enough to make the selector relevant to geometry lint
            return true;
        }
    }

    false
}
