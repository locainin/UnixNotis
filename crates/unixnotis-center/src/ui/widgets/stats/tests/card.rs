//! Statistic card presentation tests

use std::time::Duration;

use super::super::builtin::worker::BuiltinSample;
use super::super::builtin::BuiltinStat;
use super::super::style::stat_kind_css_class;
use super::support::stat_item;

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

#[gtk::test]
fn missing_card_source_renders_the_placeholder() {
    let item = stat_item(None, None);

    item.refresh_missing(Duration::from_secs(1));

    assert_eq!(item.value_label.text(), "n/a");
    assert_eq!(item.last_value.borrow().as_deref(), Some("n/a"));
}

#[gtk::test]
fn failed_builtin_sample_preserves_the_last_good_value() {
    let item = stat_item(None, Some("42%"));
    let stat =
        BuiltinStat::from_command("builtin:net:unixnotis-missing-interface").expect("builtin stat");

    item.restore_builtin_sample(BuiltinSample { stat, value: None }, Duration::from_secs(1));

    assert_eq!(item.value_label.text(), "42%");
    assert_eq!(item.last_value.borrow().as_deref(), Some("42%"));
    assert!(!item.inflight.get());
    assert!(item.builtin.borrow().is_some());
}

#[gtk::test]
fn successful_builtin_sample_replaces_a_changed_value() {
    let item = stat_item(None, Some("41%"));
    let stat = BuiltinStat::from_command("builtin:cpu").expect("builtin stat");

    item.restore_builtin_sample(
        BuiltinSample {
            stat,
            value: Some("42%".to_string()),
        },
        Duration::from_secs(1),
    );

    assert_eq!(item.value_label.text(), "42%");
    assert_eq!(item.last_value.borrow().as_deref(), Some("42%"));
    assert!(!item.inflight.get());
}
