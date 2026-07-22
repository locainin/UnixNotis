use super::skip_ui_for_zero_duration;

#[test]
fn only_zero_duration_runs_skip_ui_startup() {
    assert!(skip_ui_for_zero_duration(Some(0)));
    assert!(!skip_ui_for_zero_duration(Some(1)));
    assert!(!skip_ui_for_zero_duration(None));
}
