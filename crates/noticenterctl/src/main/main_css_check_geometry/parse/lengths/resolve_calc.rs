//! calc() resolution for geometry length parsing

use super::{parse_length_expression, CssCustomProperties, ResolvedCssValue};

pub(super) fn evaluate_calc_value(
    expression: &str,
    custom_properties: &CssCustomProperties,
    depth: usize,
) -> Option<ResolvedCssValue> {
    // calc() delegates back into the same expression parser so nested math stays consistent
    let inner = expression
        .trim()
        .strip_prefix("calc(")?
        .strip_suffix(')')?
        .trim();
    // The inner math is parsed exactly once through the shared expression grammar
    parse_length_expression(inner, custom_properties, depth + 1)
}
