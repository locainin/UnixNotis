use std::collections::HashMap;

use crate::main_css_check::main_css_check_policy;

const SMALL_INLINE_WIDTH_WARNING_THRESHOLD_PX: f32 = 64.0;

pub(in crate::main_css_check) fn should_warn_for_unmodeled_known_class(
    class_name: &str,
    properties: &[(String, String)],
) -> bool {
    // Known hooks with custom width rules should be visible once they leave stock CSS
    should_track_unmodeled_width_owner(class_name, properties)
        && !is_small_inline_badge_or_icon(class_name, properties)
        && !stock_matches_horizontal_size_rules(class_name, properties)
}

fn should_track_unmodeled_width_owner(class_name: &str, properties: &[(String, String)]) -> bool {
    if !matches!(
        class_name,
        ".unixnotis-panel-action"
            | ".unixnotis-panel-count"
            | ".unixnotis-panel-search"
            | ".unixnotis-popup-action"
            | ".unixnotis-popup-icon"
            | ".unixnotis-group-row"
            | ".unixnotis-quick-slider"
    ) {
        return false;
    }

    properties.iter().any(|(name, _)| {
        matches!(
            name.trim(),
            "width" | "min-width" | "margin" | "margin-left" | "margin-right"
        )
    })
}

fn stock_matches_horizontal_size_rules(class_name: &str, properties: &[(String, String)]) -> bool {
    let current_rules = normalized_horizontal_size_rules(properties);
    if current_rules.is_empty() {
        return true;
    }

    let Some(stock_rules) = super::baselines::stock_horizontal_size_rules().get(class_name) else {
        // Classes without a baseline should still warn when they drive width
        return false;
    };

    current_rules.iter().all(|(name, value)| {
        stock_rules
            .get(name.as_str())
            .is_some_and(|stock_value| stock_value == value)
    })
}

pub(in crate::main_css_check) fn normalized_horizontal_size_rules(
    properties: &[(String, String)],
) -> HashMap<String, String> {
    let mut current_rules = HashMap::new();
    for (name, value) in properties
        .iter()
        .filter(|(name, _)| main_css_check_policy::is_horizontal_size_property(name))
    {
        // Later duplicate properties win in GTK CSS, so the baseline check does the same
        current_rules.insert(name.trim().to_string(), value.trim().to_string());
    }
    current_rules
}

fn is_small_inline_badge_or_icon(class_name: &str, properties: &[(String, String)]) -> bool {
    if !matches!(
        class_name,
        ".unixnotis-panel-count" | ".unixnotis-popup-icon"
    ) {
        return false;
    }

    horizontal_footprint_px(&normalized_horizontal_size_rules(properties))
        .is_some_and(|width| width <= SMALL_INLINE_WIDTH_WARNING_THRESHOLD_PX)
}

fn horizontal_footprint_px(properties: &HashMap<String, String>) -> Option<f32> {
    let content_width = properties
        .get("width")
        .and_then(|value| parse_px_length(value))
        .into_iter()
        .chain(
            properties
                .get("min-width")
                .and_then(|value| parse_px_length(value)),
        )
        .fold(0.0_f32, f32::max);

    Some(
        content_width
            + horizontal_edges_px(properties, "padding")?
            + horizontal_edges_px(properties, "border")?
            + horizontal_edges_px(properties, "margin")?,
    )
}

fn horizontal_edges_px(properties: &HashMap<String, String>, family: &str) -> Option<f32> {
    let shorthand = match properties.get(family) {
        Some(value) => {
            if family == "border" {
                border_shorthand_edges_px(value)?
            } else {
                horizontal_shorthand_edges_px(value)?
            }
        }
        None => 0.0,
    };
    let width_shorthand =
        optional_horizontal_shorthand_edges_px(properties, &format!("{family}-width"))?;
    let left = optional_px_length(properties, &format!("{family}-left"))?;
    let right = optional_px_length(properties, &format!("{family}-right"))?;
    let width_left = optional_px_length(properties, &format!("{family}-left-width"))?;
    let width_right = optional_px_length(properties, &format!("{family}-right-width"))?;

    Some(shorthand + width_shorthand + left + right + width_left + width_right)
}

fn optional_horizontal_shorthand_edges_px(
    properties: &HashMap<String, String>,
    property_name: &str,
) -> Option<f32> {
    match properties.get(property_name) {
        Some(value) => horizontal_shorthand_edges_px(value),
        None => Some(0.0),
    }
}

fn optional_px_length(properties: &HashMap<String, String>, property_name: &str) -> Option<f32> {
    match properties.get(property_name) {
        Some(value) => parse_px_length(value),
        None => Some(0.0),
    }
}

fn horizontal_shorthand_edges_px(value: &str) -> Option<f32> {
    let lengths = value
        .split_whitespace()
        .map(parse_px_length)
        .collect::<Option<Vec<_>>>()?;

    match lengths.as_slice() {
        [] => Some(0.0),
        [all] => Some(all * 2.0),
        [_vertical, horizontal] => Some(horizontal * 2.0),
        [_top, right, _bottom] => Some(right * 2.0),
        [_top, right, _bottom, left] => Some(left + right),
        _ => None,
    }
}

fn border_shorthand_edges_px(value: &str) -> Option<f32> {
    let width = value.split_whitespace().find_map(parse_px_length)?;
    Some(width * 2.0)
}

fn parse_px_length(value: &str) -> Option<f32> {
    let numeric = value.trim().strip_suffix("px")?;
    if numeric
        .chars()
        .any(|ch| !(ch.is_ascii_digit() || ch == '.' || ch == '-' || ch == '+'))
    {
        return None;
    }
    numeric
        .parse::<f32>()
        .ok()
        .filter(|value| value.is_finite())
}
