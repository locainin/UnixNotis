use std::collections::HashMap;

use super::super::{resolve_compare::resolve_compare_function, ResolvedCssValue};

#[test]
fn comparison_functions_resolve_nested_length_arguments() {
    let properties = HashMap::new();

    assert_eq!(
        resolve_compare_function("min(12px, 8px)", &properties, 0),
        Some(ResolvedCssValue::Length(8.0))
    );
    assert_eq!(
        resolve_compare_function("max(12px, 8px)", &properties, 0),
        Some(ResolvedCssValue::Length(12.0))
    );
    assert_eq!(
        resolve_compare_function("clamp(4px, max(12px, 14px), 20px)", &properties, 0),
        Some(ResolvedCssValue::Length(14.0))
    );
}

#[test]
fn comparison_functions_reject_missing_or_mixed_arguments() {
    let properties = HashMap::new();

    for expression in [
        "min()",
        "max(1px, 2)",
        "clamp(1px, 2px)",
        "clamp(1px, 2, 3px)",
    ] {
        assert!(
            resolve_compare_function(expression, &properties, 0).is_none(),
            "{expression}"
        );
    }
}
