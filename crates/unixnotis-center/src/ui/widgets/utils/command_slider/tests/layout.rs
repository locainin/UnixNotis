use super::layout::slider_sublabel;

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
fn slider_sublabel_clamps_by_chars_not_bytes() {
    let label = slider_sublabel("å".repeat(40).as_str(), 0.0);

    assert_eq!(label.chars().count(), 32);
}

#[test]
fn slider_sublabel_preserves_meaningful_whitespace_inside_label() {
    assert_eq!(slider_sublabel("  low power  ", 0.0), "low power");
}
