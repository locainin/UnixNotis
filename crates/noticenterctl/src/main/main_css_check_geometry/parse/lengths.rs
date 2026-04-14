use super::super::model::HorizontalEdges;
use super::CssCustomProperties;

// Length parsing stays in one file so calc and var handling do not leak into selector logic
pub(in super::super) fn set_edge(
    edge: &mut f32,
    value: &str,
    custom_properties: &CssCustomProperties,
) {
    if let Some(parsed) = parse_single_length(value, custom_properties) {
        *edge = parsed;
    }
}

pub(in super::super) fn parse_box_edges(
    value: &str,
    custom_properties: &CssCustomProperties,
) -> Option<HorizontalEdges> {
    // CSS shorthands map to left and right edges based on token count
    let values = parse_length_tokens(value, custom_properties);
    match values.as_slice() {
        [] => None,
        [all] => Some(HorizontalEdges {
            left: *all,
            right: *all,
        }),
        [vertical, horizontal] => {
            let _ = vertical;
            Some(HorizontalEdges {
                left: *horizontal,
                right: *horizontal,
            })
        }
        [_, right, _, left] => Some(HorizontalEdges {
            left: *left,
            right: *right,
        }),
        [_, right, left] => Some(HorizontalEdges {
            left: *left,
            right: *right,
        }),
        _ => None,
    }
}

pub(in super::super) fn parse_single_length(
    value: &str,
    custom_properties: &CssCustomProperties,
) -> Option<f32> {
    let trimmed = value.trim();
    if let Some(parsed) = parse_length_expression(trimmed, custom_properties, 0) {
        return Some(parsed);
    }

    // Fall back to the first token so old shorthand behavior stays intact
    split_css_value_tokens(trimmed)
        .into_iter()
        .find_map(|token| parse_length_token(token, custom_properties, 0))
}

fn parse_length_tokens(value: &str, custom_properties: &CssCustomProperties) -> Vec<f32> {
    // Four tokens are enough for the full CSS box shorthand
    split_css_value_tokens(value)
        .into_iter()
        .filter_map(|token| parse_length_token(token, custom_properties, 0))
        .take(4)
        .collect()
}

fn parse_length_expression(
    value: &str,
    custom_properties: &CssCustomProperties,
    depth: usize,
) -> Option<f32> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    parse_length_token(trimmed, custom_properties, depth)
}

fn parse_length_token(
    token: &str,
    custom_properties: &CssCustomProperties,
    depth: usize,
) -> Option<f32> {
    if depth > 8 {
        // Recursion limits keep broken variable loops from spinning forever
        return None;
    }

    let trimmed = token.trim().trim_end_matches(',');
    if trimmed.is_empty() || trimmed.contains('%') {
        // Percentages still depend on parent geometry that this lint does not model
        return None;
    }
    if trimmed.starts_with("var(") {
        return resolve_custom_property_length(trimmed, custom_properties, depth + 1);
    }
    if trimmed.starts_with("calc(") {
        return evaluate_calc_length(trimmed, custom_properties, depth + 1);
    }
    // GTK theme sizes in this codebase are plain px-like numbers, so keep parsing simple
    let numeric = trimmed
        .strip_suffix("px")
        .or_else(|| trimmed.strip_suffix("PX"))
        .unwrap_or(trimmed);
    numeric.parse::<f32>().ok()
}

fn resolve_custom_property_length(
    expression: &str,
    custom_properties: &CssCustomProperties,
    depth: usize,
) -> Option<f32> {
    // var(--name, fallback) should keep working for simple length tokens
    let inner = expression
        .trim()
        .strip_prefix("var(")?
        .strip_suffix(')')?
        .trim();
    let (name, fallback) = split_top_level_once(inner, ',');
    let name = name.trim();
    if let Some(value) = custom_properties.get(name) {
        return parse_length_expression(value, custom_properties, depth + 1);
    }
    fallback.and_then(|value| parse_length_expression(value.trim(), custom_properties, depth + 1))
}

fn evaluate_calc_length(
    expression: &str,
    custom_properties: &CssCustomProperties,
    depth: usize,
) -> Option<f32> {
    // Geometry lint only needs simple add/subtract math for width-like values
    let inner = expression
        .trim()
        .strip_prefix("calc(")?
        .strip_suffix(')')?
        .trim();
    let mut total = 0.0_f32;
    let mut current = String::new();
    let mut sign = 1.0_f32;
    let mut paren_depth = 0u32;
    let mut bracket_depth = 0u32;
    let mut in_string = None::<char>;
    let mut saw_term = false;

    for ch in inner.chars() {
        if let Some(quote) = in_string {
            if ch == quote {
                in_string = None;
            }
            current.push(ch);
            continue;
        }

        match ch {
            '"' | '\'' => {
                in_string = Some(ch);
                current.push(ch);
            }
            '(' => {
                paren_depth = paren_depth.saturating_add(1);
                current.push(ch);
            }
            ')' => {
                paren_depth = paren_depth.saturating_sub(1);
                current.push(ch);
            }
            '[' => {
                bracket_depth = bracket_depth.saturating_add(1);
                current.push(ch);
            }
            ']' => {
                bracket_depth = bracket_depth.saturating_sub(1);
                current.push(ch);
            }
            '+' | '-' if paren_depth == 0 && bracket_depth == 0 => {
                if !current.trim().is_empty() {
                    let value =
                        parse_length_expression(current.trim(), custom_properties, depth + 1)?;
                    total += sign * value;
                    current.clear();
                    saw_term = true;
                }
                sign = if ch == '-' { -1.0 } else { 1.0 };
            }
            _ => current.push(ch),
        }
    }

    if !current.trim().is_empty() {
        total += sign * parse_length_expression(current.trim(), custom_properties, depth + 1)?;
        saw_term = true;
    }

    saw_term.then_some(total)
}

fn split_css_value_tokens(value: &str) -> Vec<&str> {
    let mut tokens = Vec::new();
    let mut start = None::<usize>;
    let mut paren_depth = 0u32;
    let mut bracket_depth = 0u32;
    let mut in_string = None::<char>;

    for (index, ch) in value.char_indices() {
        if let Some(quote) = in_string {
            if ch == quote {
                in_string = None;
            }
            if start.is_none() {
                start = Some(index);
            }
            continue;
        }

        match ch {
            '"' | '\'' => {
                if start.is_none() {
                    start = Some(index);
                }
                in_string = Some(ch);
            }
            '(' => {
                if start.is_none() {
                    start = Some(index);
                }
                paren_depth = paren_depth.saturating_add(1);
            }
            ')' => paren_depth = paren_depth.saturating_sub(1),
            '[' => {
                if start.is_none() {
                    start = Some(index);
                }
                bracket_depth = bracket_depth.saturating_add(1);
            }
            ']' => bracket_depth = bracket_depth.saturating_sub(1),
            _ if ch.is_whitespace() && paren_depth == 0 && bracket_depth == 0 => {
                if let Some(token_start) = start.take() {
                    tokens.push(value[token_start..index].trim());
                }
            }
            _ => {
                if start.is_none() {
                    start = Some(index);
                }
            }
        }
    }

    if let Some(token_start) = start {
        tokens.push(value[token_start..].trim());
    }

    tokens
        .into_iter()
        .filter(|token| !token.is_empty())
        .collect()
}

fn split_top_level_once(input: &str, separator: char) -> (&str, Option<&str>) {
    let mut paren_depth = 0u32;
    let mut bracket_depth = 0u32;
    let mut in_string = None::<char>;

    for (index, ch) in input.char_indices() {
        if let Some(quote) = in_string {
            if ch == quote {
                in_string = None;
            }
            continue;
        }

        match ch {
            '"' | '\'' => in_string = Some(ch),
            '(' => paren_depth = paren_depth.saturating_add(1),
            ')' => paren_depth = paren_depth.saturating_sub(1),
            '[' => bracket_depth = bracket_depth.saturating_add(1),
            ']' => bracket_depth = bracket_depth.saturating_sub(1),
            _ if ch == separator && paren_depth == 0 && bracket_depth == 0 => {
                let right = index + ch.len_utf8();
                return (&input[..index], Some(&input[right..]));
            }
            _ => {}
        }
    }

    (input, None)
}
