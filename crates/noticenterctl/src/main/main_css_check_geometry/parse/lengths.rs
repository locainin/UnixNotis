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
        return parsed.into_length();
    }

    // Fall back to the first token so old shorthand behavior stays intact
    split_css_value_tokens(trimmed)
        .into_iter()
        .find_map(|token| parse_length_expression(token, custom_properties, 0))
        .and_then(ResolvedCssValue::into_length)
}

fn parse_length_tokens(value: &str, custom_properties: &CssCustomProperties) -> Vec<f32> {
    // Four tokens are enough for the full CSS box shorthand
    split_css_value_tokens(value)
        .into_iter()
        .filter_map(|token| parse_length_expression(token, custom_properties, 0))
        .filter_map(ResolvedCssValue::into_length)
        .take(4)
        .collect()
}

fn parse_length_expression(
    value: &str,
    custom_properties: &CssCustomProperties,
    depth: usize,
) -> Option<ResolvedCssValue> {
    let trimmed = value.trim();
    if trimmed.is_empty() || depth > 8 {
        // Recursion limits keep broken variable loops from spinning forever
        return None;
    }

    LengthExpressionParser::new(trimmed, custom_properties, depth).parse()
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum ResolvedCssValue {
    Length(f32),
    Scalar(f32),
}

impl ResolvedCssValue {
    fn into_length(self) -> Option<f32> {
        match self {
            Self::Length(value) => Some(value),
            Self::Scalar(_) => None,
        }
    }

    fn add(self, rhs: Self) -> Option<Self> {
        match (self, rhs) {
            (Self::Length(left), Self::Length(right)) => Some(Self::Length(left + right)),
            (Self::Scalar(left), Self::Scalar(right)) => Some(Self::Scalar(left + right)),
            _ => None,
        }
    }

    fn subtract(self, rhs: Self) -> Option<Self> {
        match (self, rhs) {
            (Self::Length(left), Self::Length(right)) => Some(Self::Length(left - right)),
            (Self::Scalar(left), Self::Scalar(right)) => Some(Self::Scalar(left - right)),
            _ => None,
        }
    }

    fn multiply(self, rhs: Self) -> Option<Self> {
        match (self, rhs) {
            (Self::Scalar(left), Self::Scalar(right)) => Some(Self::Scalar(left * right)),
            (Self::Length(length), Self::Scalar(scale))
            | (Self::Scalar(scale), Self::Length(length)) => Some(Self::Length(length * scale)),
            _ => None,
        }
    }

    fn divide(self, rhs: Self) -> Option<Self> {
        match (self, rhs) {
            (_, Self::Scalar(divisor)) if divisor.abs() < f32::EPSILON => None,
            (Self::Scalar(left), Self::Scalar(right)) => Some(Self::Scalar(left / right)),
            (Self::Length(length), Self::Scalar(divisor)) => Some(Self::Length(length / divisor)),
            _ => None,
        }
    }

    fn apply_sign(self, sign: f32) -> Self {
        match self {
            Self::Length(value) => Self::Length(value * sign),
            Self::Scalar(value) => Self::Scalar(value * sign),
        }
    }
}

struct LengthExpressionParser<'a> {
    input: &'a str,
    cursor: usize,
    // Resolved custom properties are passed in so var() can stay local to the tracked selector
    custom_properties: &'a CssCustomProperties,
    // Depth keeps broken recursive tokens from looping forever
    depth: usize,
}

impl<'a> LengthExpressionParser<'a> {
    fn new(input: &'a str, custom_properties: &'a CssCustomProperties, depth: usize) -> Self {
        Self {
            input,
            cursor: 0,
            custom_properties,
            depth,
        }
    }

    fn parse(mut self) -> Option<ResolvedCssValue> {
        let value = self.parse_additive_expression()?;
        self.skip_whitespace();
        (self.cursor == self.input.len()).then_some(value)
    }

    fn parse_additive_expression(&mut self) -> Option<ResolvedCssValue> {
        let mut value = self.parse_multiplicative_expression()?;
        loop {
            self.skip_whitespace();
            if self.consume_char('+') {
                value = value.add(self.parse_multiplicative_expression()?)?;
                continue;
            }
            if self.consume_char('-') {
                value = value.subtract(self.parse_multiplicative_expression()?)?;
                continue;
            }
            break;
        }
        Some(value)
    }

    fn parse_multiplicative_expression(&mut self) -> Option<ResolvedCssValue> {
        let mut value = self.parse_factor()?;
        loop {
            self.skip_whitespace();
            if self.consume_char('*') {
                value = value.multiply(self.parse_factor()?)?;
                continue;
            }
            if self.consume_char('/') {
                value = value.divide(self.parse_factor()?)?;
                continue;
            }
            break;
        }
        Some(value)
    }

    fn parse_factor(&mut self) -> Option<ResolvedCssValue> {
        self.skip_whitespace();

        let mut sign = 1.0_f32;
        loop {
            if self.consume_char('+') {
                self.skip_whitespace();
                continue;
            }
            if self.consume_char('-') {
                sign *= -1.0;
                self.skip_whitespace();
                continue;
            }
            break;
        }

        if self.consume_char('(') {
            let value = self.parse_additive_expression()?;
            self.skip_whitespace();
            self.consume_char(')').then_some(value.apply_sign(sign))
        } else {
            let token = self.consume_token()?;
            parse_atomic_value(token, self.custom_properties, self.depth + 1)
                .map(|value| value.apply_sign(sign))
        }
    }

    fn consume_token(&mut self) -> Option<&'a str> {
        self.skip_whitespace();
        let start = self.cursor;
        let bytes = self.input.as_bytes();

        while self.cursor < bytes.len() {
            let byte = bytes[self.cursor];
            if byte.is_ascii_whitespace() || matches!(byte, b'+' | b'-' | b'*' | b'/' | b')') {
                break;
            }

            if byte == b'(' {
                self.cursor = consume_balanced_group(self.input, self.cursor)?;
                continue;
            }

            self.cursor += 1;
        }

        (self.cursor > start).then(|| self.input[start..self.cursor].trim())
    }

    fn skip_whitespace(&mut self) {
        while let Some(ch) = self.input[self.cursor..].chars().next() {
            if ch.is_whitespace() {
                self.cursor += ch.len_utf8();
            } else {
                break;
            }
        }
    }

    fn consume_char(&mut self, expected: char) -> bool {
        let Some(ch) = self.input[self.cursor..].chars().next() else {
            return false;
        };
        if ch != expected {
            return false;
        }
        self.cursor += ch.len_utf8();
        true
    }
}

fn parse_atomic_value(
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

fn resolve_custom_property_value(
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
        return parse_length_expression(value, custom_properties, depth + 1);
    }
    fallback.and_then(|value| parse_length_expression(value.trim(), custom_properties, depth + 1))
}

fn evaluate_calc_value(
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
    parse_length_expression(inner, custom_properties, depth + 1)
}

fn consume_balanced_group(input: &str, start: usize) -> Option<usize> {
    let bytes = input.as_bytes();
    let mut cursor = start;
    let mut paren_depth = 0u32;
    let mut bracket_depth = 0u32;
    let mut in_string = None::<char>;

    while cursor < bytes.len() {
        let ch = input[cursor..].chars().next()?;
        cursor += ch.len_utf8();

        if let Some(quote) = in_string {
            if ch == quote {
                in_string = None;
            }
            continue;
        }

        match ch {
            '"' | '\'' => in_string = Some(ch),
            '(' => paren_depth = paren_depth.saturating_add(1),
            ')' => {
                paren_depth = paren_depth.saturating_sub(1);
                if paren_depth == 0 && bracket_depth == 0 {
                    return Some(cursor);
                }
            }
            '[' => bracket_depth = bracket_depth.saturating_add(1),
            ']' => bracket_depth = bracket_depth.saturating_sub(1),
            _ => {}
        }
    }

    None
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
