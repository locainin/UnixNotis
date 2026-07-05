use super::super::main_css_check_geometry::{
    can_model_horizontal_size_value, CssCustomPropertyScopes,
};
use super::super::main_css_check_policy::is_horizontal_size_property;

pub(super) fn should_suppress_duplicate_property_warning(
    property: &str,
    previous: &str,
    current: &str,
    selector: &str,
    custom_properties: &CssCustomPropertyScopes,
) -> bool {
    let previous = previous.trim();
    let current = current.trim();

    if previous == current {
        return true;
    }

    let previous_is_modern = previous.contains("var(") || previous.contains("calc(");
    let current_is_modern = current.contains("var(") || current.contains("calc(");

    if previous_is_modern || !current_is_modern {
        // Only the legacy-then-modern fallback pattern is eligible for suppression
        return false;
    }

    if !is_horizontal_size_property(property) {
        // Non-layout properties keep the old quiet fallback path because geometry does not own them
        return true;
    }

    // Width-driving properties only suppress duplicates when the modern override is actually understood
    can_model_horizontal_size_value(selector, property, current, custom_properties)
}

pub(super) fn web_length_value_warning(
    property: &str,
    value: &str,
    selector: &str,
    context: Option<&str>,
    custom_properties: &CssCustomPropertyScopes,
) -> Option<String> {
    if !is_horizontal_size_property(property) {
        // Lint only escalates value-shape issues for properties that can affect width math
        return None;
    }

    if !value.contains('%')
        && can_model_horizontal_size_value(selector, property, value, custom_properties)
    {
        // Values the shared geometry parser can resolve do not need a fallback lint warning
        return None;
    }

    let hint = if value.contains('%') {
        // Percentages still depend on parent geometry that css-check does not model
        Some("uses percentage lengths, so geometry estimates may be incomplete")
    } else if contains_unmodeled_length_unit(value) {
        // GTK accepts several absolute and font-based units that geometry still cannot turn into px
        Some("uses non-px length units that css-check could not resolve reliably")
    } else if contains_compare_length_function(value) {
        // min(), max(), and clamp() are valid GTK CSS, so unresolved cases should still be visible
        Some("uses min(), max(), or clamp() in a layout value that css-check could not resolve reliably")
    } else if value.contains("calc(") {
        // This stays noisy only when the shared geometry parser could not follow the math
        Some("uses calc() in a layout value that css-check could not resolve reliably")
    } else if value.contains("var(") {
        // This stays noisy only when the shared geometry parser could not follow the token chain
        Some("uses var() in a layout value that css-check could not resolve reliably")
    } else {
        None
    }?;

    let context_note = context
        .map(|ctx| format!(" within {ctx}"))
        .unwrap_or_default();
    Some(format!(
        "property '{}' in selector '{}'{} {}",
        property, selector, context_note, hint
    ))
}

fn contains_compare_length_function(value: &str) -> bool {
    // A tiny string check is enough here because parsing only happens after the shared
    // geometry parser fails to resolve the value
    let lowered = value.to_ascii_lowercase();
    lowered.contains("min(") || lowered.contains("max(") || lowered.contains("clamp(")
}

fn contains_unmodeled_length_unit(value: &str) -> bool {
    let lowered = value.to_ascii_lowercase();
    let chars = lowered.char_indices().collect::<Vec<_>>();
    let mut index = 0usize;

    while index < chars.len() {
        let (_, ch) = chars[index];
        let next_is_digit = chars
            .get(index + 1)
            .map(|(_, next)| next.is_ascii_digit())
            .unwrap_or(false);

        if !(ch.is_ascii_digit() || (ch == '.' && next_is_digit)) {
            index += 1;
            continue;
        }

        // Scan the numeric part first so the next byte range can be treated like one unit token
        index += 1;
        while let Some((_, current)) = chars.get(index) {
            if current.is_ascii_digit() || *current == '.' {
                index += 1;
                continue;
            }
            break;
        }

        let unit_start = index;
        while let Some((_, current)) = chars.get(index) {
            if current.is_ascii_alphabetic() {
                index += 1;
                continue;
            }
            break;
        }

        if unit_start == index {
            continue;
        }

        let start_byte = chars[unit_start].0;
        let end_byte = chars
            .get(index)
            .map(|(offset, _)| *offset)
            .unwrap_or_else(|| lowered.len());
        let unit = &lowered[start_byte..end_byte];

        if unit != "px" {
            // GTK accepts several units here, but geometry still only knows how to estimate px
            return true;
        }
    }

    false
}

pub(super) fn line_column_for_offset(contents: &str, offset: usize) -> (usize, usize) {
    // Byte offsets are enough here because css-check already works on UTF-8 source slices
    let mut line = 1usize;
    let mut column = 1usize;
    for (index, ch) in contents.char_indices() {
        if index >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }
    (line, column)
}
