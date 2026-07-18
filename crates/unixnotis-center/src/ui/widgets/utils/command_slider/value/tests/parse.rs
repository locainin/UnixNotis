#![allow(
    clippy::float_cmp,
    reason = "the parser produces exact values for these bounded decimal inputs"
)]

use unixnotis_core::NumericParseMode;

use super::{parse_muted, parse_numeric};

#[test]
fn parse_numeric_prefers_percent_tokens_and_clamps_to_range() {
    assert_eq!(
        parse_numeric("raw 10 current 75%", 0.0, 100.0, NumericParseMode::Auto),
        Some(75.0)
    );
    assert_eq!(
        parse_numeric("value 120", 0.0, 100.0, NumericParseMode::Percent),
        Some(100.0)
    );
}

#[test]
fn parse_numeric_applies_auto_and_ratio_modes() {
    assert_eq!(
        parse_numeric("value 0.42", 0.0, 100.0, NumericParseMode::Auto),
        Some(42.0)
    );
    assert_eq!(
        parse_numeric("value 0.25", 0.0, 100.0, NumericParseMode::Ratio),
        Some(25.0)
    );
    assert_eq!(
        parse_numeric("value 2", 0.0, 100.0, NumericParseMode::Auto),
        Some(2.0)
    );
    assert_eq!(
        parse_numeric("value 5.5", 0.0, 100.0, NumericParseMode::Auto),
        Some(5.5)
    );
    assert_eq!(
        parse_numeric("value 0.5%", 0.0, 100.0, NumericParseMode::Auto),
        Some(0.5)
    );
}

#[test]
fn parse_numeric_rejects_output_without_a_number() {
    assert_eq!(
        parse_numeric("value unavailable", 0.0, 100.0, NumericParseMode::Auto),
        None
    );
}

#[test]
fn parse_muted_matches_supported_markers_without_case_sensitivity() {
    assert!(parse_muted("Audio MUTED"));
    assert!(parse_muted("Mute: YES"));
    assert!(!parse_muted("active"));
}
