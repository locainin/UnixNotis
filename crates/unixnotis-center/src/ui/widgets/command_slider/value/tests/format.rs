use super::{format_command_value, format_display_value};

#[test]
fn format_display_value_uses_whole_percent_text() {
    assert_eq!(format_display_value(42.4), "42%");
    assert_eq!(format_display_value(42.6), "43%");
}

#[test]
fn format_command_value_keeps_fractional_precision_from_step() {
    assert_eq!(format_command_value(12.5, 0.5), "12.5");
    assert_eq!(format_command_value(12.25, 0.25), "12.25");
    assert_eq!(format_command_value(12.125, 0.125), "12.125");
    assert_eq!(format_command_value(1.234_56, 0.01), "1.23");
    assert_eq!(format_command_value(1.234_56, 0.001), "1.235");
}

#[test]
fn format_command_value_trims_integer_suffix_when_step_is_whole() {
    assert_eq!(format_command_value(42.0, 1.0), "42");
    assert_eq!(format_command_value(42.0, 10.0), "42");
}

#[test]
fn format_command_value_trims_fractional_trailing_zeroes() {
    assert_eq!(format_command_value(12.5, 0.25), "12.5");
    assert_eq!(format_command_value(12.0, 0.25), "12");
}

#[test]
fn format_command_value_falls_back_for_invalid_steps() {
    assert_eq!(format_command_value(42.9, 0.0), "43");
    assert_eq!(format_command_value(42.9, f64::NAN), "43");
}
