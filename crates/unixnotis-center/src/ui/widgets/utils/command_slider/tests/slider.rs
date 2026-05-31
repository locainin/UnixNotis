use super::layout::slider_sublabel;
use super::refresh::SliderRefreshGate;
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
fn slider_value_changed_uses_step_sized_tolerance() {
    assert_eq!(slider_value_tolerance(0.1), 0.05);
    assert!(!slider_value_changed(50.0, 50.04, 0.1));
    assert!(slider_value_changed(50.0, 50.06, 0.1));
}

#[test]
fn slider_sublabel_uses_numeric_fallback_when_unset() {
    assert_eq!(slider_sublabel("", 25.0), "25%");
}

#[test]
fn slider_sublabel_trims_and_clamps_configured_text() {
    let label = slider_sublabel("  abcdefghijklmnopqrstuvwxyz0123456789  ", 0.0);

    assert_eq!(label, "abcdefghijklmnopqrstuvwxyz012345");
}

#[test]
fn refresh_gate_queues_one_trailing_refresh() {
    let gate = SliderRefreshGate::new();

    assert!(gate.begin_or_queue());
    assert!(!gate.begin_or_queue());
    assert!(gate.finish());
}

#[test]
fn refresh_gate_clears_pending_after_finish() {
    let gate = SliderRefreshGate::new();

    assert!(gate.begin_or_queue());
    assert!(!gate.begin_or_queue());
    assert!(gate.finish());
    assert!(!gate.finish());
    assert!(gate.begin_or_queue());
}

#[test]
fn refresh_gate_does_not_stack_multiple_pending_runs() {
    let gate = SliderRefreshGate::new();

    assert!(gate.begin_or_queue());
    assert!(!gate.begin_or_queue());
    assert!(!gate.begin_or_queue());
    assert!(gate.finish());
    assert!(!gate.finish());
}
