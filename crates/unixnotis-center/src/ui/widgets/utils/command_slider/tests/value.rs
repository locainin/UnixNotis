use super::value::{format_command_value, slider_value_changed, slider_value_tolerance};

#[test]
fn format_command_value_keeps_fractional_precision_from_step() {
    assert_eq!(format_command_value(12.5, 0.5), "12.5");
    assert_eq!(format_command_value(12.25, 0.25), "12.25");
    assert_eq!(format_command_value(12.125, 0.125), "12.125");
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

#[test]
fn slider_value_changed_uses_step_sized_tolerance() {
    assert_eq!(slider_value_tolerance(0.1), 0.05);
    assert!(!slider_value_changed(50.0, 50.04, 0.1));
    assert!(slider_value_changed(50.0, 50.06, 0.1));
}

#[test]
fn slider_value_tolerance_handles_invalid_steps() {
    assert_eq!(slider_value_tolerance(0.0), 1e-6);
    assert_eq!(slider_value_tolerance(f64::INFINITY), 1e-6);
}
