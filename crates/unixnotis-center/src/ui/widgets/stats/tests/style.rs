//! Statistic style tests

use super::stat_kind_css_class;

#[test]
fn card_kind_class_normalizes_theme_tokens() {
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
