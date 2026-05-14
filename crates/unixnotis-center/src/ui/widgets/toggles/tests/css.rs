use super::toggle_kind_css_class;

#[test]
fn kind_css_class_sanitizes_to_stable_token() {
    assert_eq!(
        toggle_kind_css_class("WiFi"),
        Some("unixnotis-toggle-kind-wifi".to_string())
    );
    assert_eq!(
        toggle_kind_css_class("airplane_mode"),
        Some("unixnotis-toggle-kind-airplane-mode".to_string())
    );
    assert_eq!(toggle_kind_css_class("  !!!  "), None);
}
