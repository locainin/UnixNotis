//! Typed arithmetic parser for CSS length expressions

use super::tokenize::consume_balanced_group;
use super::units::parse_atomic_value;
use super::CssCustomProperties;

pub(in crate::css_check::geometry::parse) fn parse_length_expression(
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
pub(in crate::css_check::geometry::parse) enum ResolvedCssValue {
    // Length values may participate in compatible arithmetic and become geometry
    Length(f32),
    // Scalars are valid only as intermediate scale or divisor values
    Scalar(f32),
}

impl ResolvedCssValue {
    pub(super) const fn into_length(self) -> Option<f32> {
        match self {
            Self::Length(value) => Some(value),
            // Plain scalars only make sense while calc math is still in progress
            Self::Scalar(_) => None,
        }
    }

    fn add(self, rhs: Self) -> Option<Self> {
        // Addition cannot mix a dimensioned length with a scalar
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
        // Multiplication accepts one dimensioned side at most
        match (self, rhs) {
            (Self::Scalar(left), Self::Scalar(right)) => Some(Self::Scalar(left * right)),
            (Self::Length(length), Self::Scalar(scale))
            | (Self::Scalar(scale), Self::Length(length)) => Some(Self::Length(length * scale)),
            _ => None,
        }
    }

    fn divide(self, rhs: Self) -> Option<Self> {
        // Only scalar divisors preserve a valid CSS length dimension
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

    pub(super) const fn min_with(self, rhs: Self) -> Option<Self> {
        match (self, rhs) {
            (Self::Length(left), Self::Length(right)) => Some(Self::Length(left.min(right))),
            (Self::Scalar(left), Self::Scalar(right)) => Some(Self::Scalar(left.min(right))),
            _ => None,
        }
    }

    pub(super) const fn max_with(self, rhs: Self) -> Option<Self> {
        match (self, rhs) {
            (Self::Length(left), Self::Length(right)) => Some(Self::Length(left.max(right))),
            (Self::Scalar(left), Self::Scalar(right)) => Some(Self::Scalar(left.max(right))),
            _ => None,
        }
    }

    pub(super) fn clamp_between(self, lower: Self, upper: Self) -> Option<Self> {
        // clamp() keeps the value inside the two bounds once all three share one type
        lower.max_with(self)?.min_with(upper)
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
    const fn new(input: &'a str, custom_properties: &'a CssCustomProperties, depth: usize) -> Self {
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
        // Partial parses are rejected so geometry only trusts whole expressions
        (self.cursor == self.input.len()).then_some(value)
    }

    fn parse_additive_expression(&mut self) -> Option<ResolvedCssValue> {
        let mut value = self.parse_multiplicative_expression()?;
        loop {
            self.skip_whitespace();
            if self.consume_char('+') {
                // Addition stays left-associative like normal CSS calc evaluation
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
        // Multiplication binds more tightly than the additive parser above it
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

        // Repeated unary signs are folded before reading a group or atomic token
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
        // Cursor positions stay on UTF-8 boundaries because non-ASCII bytes are token content
        let start = self.cursor;
        let bytes = self.input.as_bytes();

        while self.cursor < bytes.len() {
            let byte = bytes[self.cursor];
            if byte.is_ascii_whitespace() || matches!(byte, b'+' | b'-' | b'*' | b'/' | b')') {
                break;
            }

            if byte == b'(' {
                // Nested groups are consumed whole so inner operators do not split the token
                self.cursor = consume_balanced_group(self.input, self.cursor)?;
                continue;
            }

            self.cursor += 1;
        }

        (self.cursor > start).then(|| self.input[start..self.cursor].trim())
    }

    fn skip_whitespace(&mut self) {
        // Character iteration handles every Unicode whitespace boundary safely
        while let Some(ch) = self.input[self.cursor..].chars().next() {
            if ch.is_whitespace() {
                self.cursor += ch.len_utf8();
            } else {
                break;
            }
        }
    }

    fn consume_char(&mut self, expected: char) -> bool {
        // Operators are consumed only when the next complete character matches
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
