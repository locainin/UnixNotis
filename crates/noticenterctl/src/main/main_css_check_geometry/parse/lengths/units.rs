//! Atomic value parsing for geometry length expressions

use super::{
    resolve_calc::evaluate_calc_value, resolve_var::resolve_custom_property_value,
    CssCustomProperties, ResolvedCssValue,
};

pub(super) fn parse_atomic_value(
    token: &str,
    custom_properties: &CssCustomProperties,
    depth: usize,
) -> Option<ResolvedCssValue> {
    let trimmed = token.trim().trim_end_matches(',');
    if trimmed.is_empty() || trimmed.contains('%') {
        // Percentages still depend on parent geometry that this lint does not model
        return None;
    }

    if trimmed.starts_with("var(") {
        // var() stays ahead of raw number parsing so custom properties can nest calc values
        return resolve_custom_property_value(trimmed, custom_properties, depth);
    }
    if trimmed.starts_with("calc(") {
        return evaluate_calc_value(trimmed, custom_properties, depth);
    }

    parse_numeric_or_length(trimmed)
}

fn parse_numeric_or_length(token: &str) -> Option<ResolvedCssValue> {
    let trimmed = token.trim();
    // Geometry only needs px lengths and plain scalars for calc math
    let numeric = trimmed
        .strip_suffix("px")
        .or_else(|| trimmed.strip_suffix("PX"));

    if let Some(value) = numeric.and_then(|value| value.parse::<f32>().ok()) {
        return Some(ResolvedCssValue::Length(value));
    }

    trimmed.parse::<f32>().ok().map(ResolvedCssValue::Scalar)
}
