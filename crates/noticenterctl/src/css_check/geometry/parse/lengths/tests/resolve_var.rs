use std::collections::HashMap;

use super::super::{resolve_var::resolve_custom_property_value, ResolvedCssValue};

#[test]
fn custom_property_resolution_prefers_the_defined_value() {
    let properties = HashMap::from([("--width".to_string(), "calc(10px + 2px)".to_string())]);

    assert_eq!(
        resolve_custom_property_value("var(--width, 30px)", &properties, 0),
        Some(ResolvedCssValue::Length(12.0))
    );
}

#[test]
fn custom_property_resolution_uses_a_nested_fallback() {
    let properties = HashMap::new();

    assert_eq!(
        resolve_custom_property_value("var(--missing, max(12px, 18px))", &properties, 0),
        Some(ResolvedCssValue::Length(18.0))
    );
}

#[test]
fn custom_property_resolution_rejects_missing_or_malformed_fallbacks() {
    let properties = HashMap::new();

    for expression in ["var(--missing)", "var(--missing, 12px", "var()"] {
        assert!(
            resolve_custom_property_value(expression, &properties, 0).is_none(),
            "{expression}"
        );
    }
}
