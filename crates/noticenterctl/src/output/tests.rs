use super::super::output_gate::{allow_full_output, warn_full_requires_diagnostic};

#[test]
fn full_output_gate_requires_request_and_diagnostic_mode() {
    assert!(allow_full_output(true, true));
    assert!(!allow_full_output(true, false));
    assert!(!allow_full_output(false, true));
    assert!(!allow_full_output(false, false));
}

#[test]
fn full_output_warning_only_when_full_was_requested_without_diagnostic_mode() {
    assert!(warn_full_requires_diagnostic(true, false));
    assert!(!warn_full_requires_diagnostic(true, true));
    assert!(!warn_full_requires_diagnostic(false, false));
    assert!(!warn_full_requires_diagnostic(false, true));
}
