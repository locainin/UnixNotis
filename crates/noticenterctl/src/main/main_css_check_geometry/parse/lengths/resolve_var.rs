//! var() resolution for geometry length parsing

use super::{
    parse_length_expression, tokenize::split_top_level_once, CssCustomProperties, ResolvedCssValue,
};

pub(super) fn resolve_custom_property_value(
    expression: &str,
    custom_properties: &CssCustomProperties,
    depth: usize,
) -> Option<ResolvedCssValue> {
    // var(--name, fallback) should keep working for nested calc and fallback chains
    let inner = expression
        .trim()
        .strip_prefix("var(")?
        .strip_suffix(')')?
        .trim();
    let (name, fallback) = split_top_level_once(inner, ',');
    let name = name.trim();
    if let Some(value) = custom_properties.get(name) {
        // Resolved properties recurse through the same parser so nested calc stays supported
        return parse_length_expression(value, custom_properties, depth + 1);
    }
    fallback.and_then(|value| parse_length_expression(value.trim(), custom_properties, depth + 1))
}
