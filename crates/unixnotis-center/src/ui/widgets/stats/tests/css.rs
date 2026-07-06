use super::stat_kind_css_class;

#[test]
fn stat_kind_css_class_sanitizes_to_stable_token() {
    assert_eq!(
        stat_kind_css_class("RAM"),
        Some("unixnotis-stat-kind-ram".to_string())
    );
    assert_eq!(
        stat_kind_css_class("RAM %#$ Thing"),
        Some("unixnotis-stat-kind-ram-thing".to_string())
    );
    assert_eq!(stat_kind_css_class("  !!!  "), None);
}
