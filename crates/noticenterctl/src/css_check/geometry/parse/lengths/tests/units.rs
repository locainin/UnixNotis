use std::collections::HashMap;

use super::super::{units::parse_atomic_value, ResolvedCssValue};

#[test]
fn atomic_values_distinguish_pixel_lengths_from_scalars() {
    let properties = HashMap::new();

    assert_eq!(
        parse_atomic_value("12PX", &properties, 0),
        Some(ResolvedCssValue::Length(12.0))
    );
    assert_eq!(
        parse_atomic_value("2.5", &properties, 0),
        Some(ResolvedCssValue::Scalar(2.5))
    );
}

#[test]
fn atomic_values_reject_percentages_and_unknown_units() {
    let properties = HashMap::new();

    for value in ["", "80%", "1rem", "unknown"] {
        assert!(
            parse_atomic_value(value, &properties, 0).is_none(),
            "{value}"
        );
    }
}
