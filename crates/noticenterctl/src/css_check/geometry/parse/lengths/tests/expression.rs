use std::collections::HashMap;

use super::super::{parse_length_expression, ResolvedCssValue};

#[test]
fn arithmetic_parser_preserves_precedence_and_parentheses() {
    let properties = HashMap::new();

    assert_eq!(
        parse_length_expression("2 * 3px + 4px", &properties, 0),
        Some(ResolvedCssValue::Length(10.0))
    );
    assert_eq!(
        parse_length_expression("2 * (3px + 4px)", &properties, 0),
        Some(ResolvedCssValue::Length(14.0))
    );
}

#[test]
fn arithmetic_parser_rejects_invalid_dimensions_and_division_by_zero() {
    let properties = HashMap::new();

    for expression in ["10px + 2", "10px * 2px", "10px / 0", "10px trailing"] {
        assert!(
            parse_length_expression(expression, &properties, 0).is_none(),
            "{expression}"
        );
    }
}

#[test]
fn arithmetic_parser_limits_recursive_resolution_depth() {
    let properties = HashMap::new();

    assert!(parse_length_expression("12px", &properties, 9).is_none());
}
