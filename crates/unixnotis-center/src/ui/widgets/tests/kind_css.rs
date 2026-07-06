use super::widget_kind_css_class;

#[test]
fn kind_css_class_sanitizes_to_stable_token() {
    assert_eq!(
        widget_kind_css_class("unixnotis-toggle-kind-", "WiFi"),
        Some("unixnotis-toggle-kind-wifi".to_string())
    );
    assert_eq!(
        widget_kind_css_class("unixnotis-stat-kind-", "RAM %#$ Thing"),
        Some("unixnotis-stat-kind-ram-thing".to_string())
    );
    assert_eq!(
        widget_kind_css_class("unixnotis-stat-kind-", "  !!!  "),
        None
    );
}
