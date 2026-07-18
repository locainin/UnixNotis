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
fn parse_numeric_preserves_signed_slider_values() {
    assert_eq!(
        parse_numeric("-5", -12.5, 12.5, NumericParseMode::Percent),
        Some(-5.0)
    );
    assert_eq!(
        parse_numeric("-0.5", -100.0, 100.0, NumericParseMode::Ratio),
        Some(-50.0)
    );
    assert_eq!(
        parse_numeric("-0.5%", -100.0, 100.0, NumericParseMode::Auto),
        Some(-0.5)
    );
    assert_eq!(
        parse_numeric("gain -6.0 dB", -12.5, 12.5, NumericParseMode::Percent),
        Some(-6.0)
    );
}

#[test]
fn parse_numeric_handles_explicit_signs_and_scientific_notation() {
    assert_eq!(
        parse_numeric("gain +5", -12.5, 12.5, NumericParseMode::Percent),
        Some(5.0)
    );
    assert_eq!(
        parse_numeric("gain 1.25e1", -20.0, 20.0, NumericParseMode::Percent),
        Some(12.5)
    );
    assert_eq!(
        parse_numeric("-5e-1", -100.0, 100.0, NumericParseMode::Ratio),
        Some(-50.0)
    );
}

#[test]
fn parse_numeric_accepts_leading_dot_decimals() {
    assert_eq!(
        parse_numeric(".5", -100.0, 100.0, NumericParseMode::Percent),
        Some(0.5)
    );
    assert_eq!(
        parse_numeric("-.5", -100.0, 100.0, NumericParseMode::Ratio),
        Some(-50.0)
    );
}

#[test]
fn parse_numeric_treats_embedded_signs_as_token_separators() {
    assert_eq!(
        parse_numeric("range 1-2", -100.0, 100.0, NumericParseMode::Percent),
        Some(2.0)
    );
    assert_eq!(
        parse_numeric("value_-5", -100.0, 100.0, NumericParseMode::Percent),
        Some(5.0)
    );
}

#[test]
fn parse_numeric_auto_mode_does_not_scale_negative_decimals() {
    assert_eq!(
        parse_numeric("value -0.5", -100.0, 100.0, NumericParseMode::Auto),
        Some(-0.5)
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
