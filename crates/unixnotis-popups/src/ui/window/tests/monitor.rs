use super::output_alias_matches;

#[test]
fn output_alias_matches_connector_without_case_sensitivity() {
    assert!(output_alias_matches(Some("DP-1"), None, "dp-1"));
}

#[test]
fn output_alias_falls_back_to_monitor_model() {
    assert!(output_alias_matches(
        None,
        Some("Studio Display"),
        "studio display"
    ));
}

#[test]
fn output_alias_rejects_empty_and_unknown_names() {
    assert!(!output_alias_matches(Some("DP-1"), Some("Panel"), "  "));
    assert!(!output_alias_matches(
        Some("DP-1"),
        Some("Panel"),
        "HDMI-A-1"
    ));
}
