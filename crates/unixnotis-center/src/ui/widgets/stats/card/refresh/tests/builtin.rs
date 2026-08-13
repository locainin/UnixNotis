//! Built-in statistic card refresh tests

use std::time::Duration;

use super::support::stat_item;
use crate::ui::widgets::stats::builtin::worker::BuiltinSample;
use crate::ui::widgets::stats::builtin::BuiltinStat;

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
