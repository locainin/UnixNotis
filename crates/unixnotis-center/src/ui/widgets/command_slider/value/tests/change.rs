#![allow(
    clippy::float_cmp,
    reason = "the tolerance helper returns exact configured constants for these finite inputs"
)]

use super::{slider_value_changed, slider_value_tolerance};

#[test]
fn slider_value_changed_uses_step_sized_tolerance() {
    assert_eq!(slider_value_tolerance(0.1), 0.05);
    assert!(!slider_value_changed(50.0, 50.04, 0.1));
    assert!(slider_value_changed(50.0, 50.06, 0.1));
    assert!(!slider_value_changed(0.0, 0.5, 1.0));
}

#[test]
fn slider_value_tolerance_handles_invalid_steps() {
    assert_eq!(slider_value_tolerance(0.0), 1e-6);
    assert_eq!(slider_value_tolerance(f64::INFINITY), 1e-6);
}
