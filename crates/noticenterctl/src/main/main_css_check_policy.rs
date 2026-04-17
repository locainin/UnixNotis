//! Shared css-check policy for GTK CSS support and geometry rules

use unixnotis_core::GTK_CSS_CUSTOM_PROPERTIES_MIN_VERSION_LABEL;

pub(super) fn is_horizontal_size_property(name: &str) -> bool {
    // Only width-driving properties belong here
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

pub(super) fn is_vertical_size_property(name: &str) -> bool {
    // Only height-driving properties belong here
    matches!(
        name.trim(),
        "height"
            | "min-height"
            | "margin"
            | "margin-top"
            | "margin-bottom"
            | "padding"
            | "padding-top"
            | "padding-bottom"
            | "border"
            | "border-width"
            | "border-top"
            | "border-top-width"
            | "border-bottom"
            | "border-bottom-width"
    )
}

pub(super) fn is_complex_geometry_warning_property(name: &str) -> bool {
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

pub(super) fn parsing_error_hint(line_text: &str) -> Option<String> {
    // Trim once so the checks stay simple
    let trimmed = line_text.trim();
    if trimmed.contains('%') {
        // GTK size rules are stricter than web CSS
        return Some(
            "percentage sizing is still property-dependent in GTK; if parsing failed here, use a fixed value or a simpler expression"
                .to_string(),
        );
    }
    if trimmed.contains("calc(") {
        // calc() is valid in GTK4, but broken unit mixes still need a useful hint
        return Some(
            "GTK supports calc() in many places, but this property/value pair is still invalid here"
                .to_string(),
        );
    }
    if trimmed.contains("var(") {
        // The minimum version note lives in one shared place so installer and checker stay aligned
        return Some(format!(
            "custom properties need {GTK_CSS_CUSTOM_PROPERTIES_MIN_VERSION_LABEL}, and the referenced token still has to expand to a valid value here"
        ));
    }
    None
}
