//! Statistic card dispatch tests

use std::time::Duration;

use super::support::stat_item;

#[gtk::test]
fn missing_card_source_renders_the_placeholder() {
    let item = stat_item(None, None);

    item.refresh_missing(Duration::from_secs(1));

    assert_eq!(item.value_label.text(), "n/a");
    assert_eq!(item.last_value.borrow().as_deref(), Some("n/a"));
}
