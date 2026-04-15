//! min(), max(), and clamp() resolution for geometry length parsing

use super::{
    parse_length_expression, tokenize::split_top_level_list, CssCustomProperties, ResolvedCssValue,
};

pub(super) fn resolve_compare_function(
    expression: &str,
    custom_properties: &CssCustomProperties,
    depth: usize,
) -> Option<ResolvedCssValue> {
    let trimmed = expression.trim();

    if let Some(inner) = trimmed
        .strip_prefix("min(")
        .and_then(|value| value.strip_suffix(')'))
    {
        return resolve_min_or_max(inner, custom_properties, depth, CompareMode::Min);
    }

    if let Some(inner) = trimmed
        .strip_prefix("max(")
        .and_then(|value| value.strip_suffix(')'))
    {
        return resolve_min_or_max(inner, custom_properties, depth, CompareMode::Max);
    }

    let inner = trimmed.strip_prefix("clamp(")?.strip_suffix(')')?.trim();
    let args = split_top_level_list(inner, ',');
    if args.len() != 3 {
        return None;
    }

    // clamp(min, value, max) is just nested compare logic once the args are resolved
    let lower = parse_length_expression(args[0], custom_properties, depth + 1)?;
    let value = parse_length_expression(args[1], custom_properties, depth + 1)?;
    let upper = parse_length_expression(args[2], custom_properties, depth + 1)?;
    value.clamp_between(lower, upper)
}

#[derive(Clone, Copy)]
enum CompareMode {
    Min,
    Max,
}

fn resolve_min_or_max(
    inner: &str,
    custom_properties: &CssCustomProperties,
    depth: usize,
    mode: CompareMode,
) -> Option<ResolvedCssValue> {
    let mut values = split_top_level_list(inner, ',')
        .into_iter()
        .map(|value| parse_length_expression(value, custom_properties, depth + 1))
        .collect::<Option<Vec<_>>>()?
        .into_iter();

    let mut resolved = values.next()?;
    for value in values {
        resolved = match mode {
            CompareMode::Min => resolved.min_with(value)?,
            CompareMode::Max => resolved.max_with(value)?,
        };
    }
    Some(resolved)
}
