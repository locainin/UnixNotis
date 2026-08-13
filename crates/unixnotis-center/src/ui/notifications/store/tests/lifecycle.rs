use std::collections::VecDeque;
use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};

use super::drain_order_over_limit;

use crate::ui::notifications::test_support as support;

fn ordered_ids(ids: &[u32]) -> VecDeque<u32> {
    ids.iter().copied().collect()
}

#[test]
fn drain_order_over_limit_removes_oldest_ids_from_back() {
    let mut order = ordered_ids(&[4, 3, 2, 1]);

    let drained = drain_order_over_limit(&mut order, 2);

    assert_eq!(drained, vec![1, 2]);
    assert_eq!(order, ordered_ids(&[4, 3]));
}

#[test]
fn drain_order_over_limit_keeps_exact_capacity() {
    let mut order = ordered_ids(&[3, 2, 1]);

    let drained = drain_order_over_limit(&mut order, 3);

    assert!(drained.is_empty());
    assert_eq!(order, ordered_ids(&[3, 2, 1]));
}

#[test]
fn drain_order_over_limit_keeps_under_capacity() {
    let mut order = ordered_ids(&[2, 1]);

    let drained = drain_order_over_limit(&mut order, 5);

    assert!(drained.is_empty());
    assert_eq!(order, ordered_ids(&[2, 1]));
}

#[test]
fn drain_order_over_limit_zero_capacity_drains_every_id() {
    let mut order = ordered_ids(&[3, 2, 1]);

    let drained = drain_order_over_limit(&mut order, 0);

    assert_eq!(drained, vec![3, 2, 1]);
    assert!(order.is_empty());
}

#[test]
fn drain_order_over_limit_zero_capacity_accepts_empty_order() {
    let mut order = VecDeque::new();

    let drained = drain_order_over_limit(&mut order, 0);

    assert!(drained.is_empty());
    assert!(order.is_empty());
}

#[gtk::test]
fn seed_preserves_existing_group_expansion_for_surviving_groups() {
    let mut list = support::make_list();
    list.group_expanded
        .insert(Rc::<str>::from("test:terminal"), true);

    list.seed(
        vec![
            support::notification(1, "Terminal"),
            support::notification(2, "Terminal"),
        ],
        vec![support::notification(3, "History")],
    );

    assert_eq!(list.group_expanded.get("test:terminal"), Some(&true));
    assert_eq!(list.total_count(), 3);
    assert_eq!(list.active_order, ordered_ids(&[2, 1]));
    assert_eq!(list.history_order, ordered_ids(&[3]));
    assert!(list.needs_rebuild());
}

#[gtk::test]
fn disconnect_reset_keeps_group_expansion_until_reconnect_seed() {
    let mut list = support::make_list();
    list.seed(
        vec![
            support::notification(1, "Terminal"),
            support::notification(2, "Terminal"),
        ],
        Vec::new(),
    );
    list.group_expanded
        .insert(Rc::<str>::from("test:terminal"), true);

    list.clear_for_disconnect();

    assert!(list.entries.is_empty());
    assert_eq!(list.group_expanded.get("test:terminal"), Some(&true));

    list.seed(
        vec![
            support::notification(3, "Terminal"),
            support::notification(4, "Terminal"),
        ],
        Vec::new(),
    );

    assert_eq!(list.group_expanded.get("test:terminal"), Some(&true));
}

#[gtk::test]
fn seed_trims_to_current_limits() {
    let mut list = support::make_list();
    list.max_active = 1;
    list.max_entries = 1;

    list.seed(
        vec![
            support::notification(1, "Terminal"),
            support::notification(2, "Terminal"),
        ],
        vec![
            support::notification(3, "History"),
            support::notification(4, "History"),
        ],
    );

    assert_eq!(list.active_order, ordered_ids(&[2]));
    assert_eq!(list.history_order, ordered_ids(&[4]));
    assert!(!list.entries.contains_key(&1));
    assert!(!list.entries.contains_key(&3));
}

#[gtk::test]
fn apply_limits_trims_active_notifications_when_limit_changes() {
    let mut list = support::make_list();
    list.seed(
        vec![
            support::notification(1, "Terminal"),
            support::notification(2, "Terminal"),
            support::notification(3, "Terminal"),
        ],
        Vec::new(),
    );
    list.needs_rebuild = false;

    list.apply_limits(2, 10);

    assert_eq!(list.active_order, ordered_ids(&[3, 2]));
    assert!(!list.entries.contains_key(&1));
    assert!(list.needs_rebuild());
}

#[gtk::test]
fn apply_limits_trims_history_notifications_when_limit_changes() {
    let mut list = support::make_list();
    list.seed(
        Vec::new(),
        vec![
            support::notification(1, "History"),
            support::notification(2, "History"),
            support::notification(3, "History"),
        ],
    );
    list.needs_rebuild = false;

    list.apply_limits(10, 2);

    assert_eq!(list.history_order, ordered_ids(&[3, 2]));
    assert!(!list.entries.contains_key(&1));
    assert!(list.needs_rebuild());
}

#[gtk::test]
fn apply_limits_same_values_keeps_state_unchanged() {
    let mut list = support::make_list();
    list.seed(vec![support::notification(1, "Terminal")], Vec::new());
    list.needs_rebuild = false;

    list.apply_limits(10, 10);

    assert_eq!(list.active_order, ordered_ids(&[1]));
    assert!(!list.needs_rebuild());
}

#[gtk::test]
fn remove_entry_drops_entry_from_all_orders() {
    let mut list = support::make_list();
    list.seed(
        vec![support::notification(1, "Terminal")],
        vec![support::notification(2, "History")],
    );

    list.remove_entry(1);
    list.remove_entry(2);

    assert!(list.entries.is_empty());
    assert!(list.active_order.is_empty());
    assert!(list.history_order.is_empty());
}

#[gtk::test]
fn remove_entry_drops_active_order_without_touching_history_order() {
    let mut list = support::make_list();
    list.seed(
        vec![
            support::notification(1, "Terminal"),
            support::notification(2, "Terminal"),
        ],
        vec![support::notification(3, "History")],
    );

    list.remove_entry(1);

    assert_eq!(list.active_order, ordered_ids(&[2]));
    assert_eq!(list.history_order, ordered_ids(&[3]));
    assert!(!list.entries.contains_key(&1));
}

#[gtk::test]
fn remove_entry_drops_history_order_without_touching_active_order() {
    let mut list = support::make_list();
    list.seed(
        vec![support::notification(1, "Terminal")],
        vec![
            support::notification(2, "History"),
            support::notification(3, "History"),
        ],
    );

    list.remove_entry(2);

    assert_eq!(list.active_order, ordered_ids(&[1]));
    assert_eq!(list.history_order, ordered_ids(&[3]));
    assert!(!list.entries.contains_key(&2));
}

#[gtk::test]
fn insert_entry_records_recent_local_timestamp() {
    let mut list = support::make_list();
    let before = super::now_millis();
    let key = list.insert_entry(support::notification(9, "Terminal"), true);
    let after = super::now_millis();

    let entry = list.entries.get(&9).expect("entry should be stored");
    assert_eq!(key.as_ref(), "test:terminal");
    assert!(entry.received_at_ms >= before);
    assert!(entry.received_at_ms <= after);
}

#[gtk::test]
fn insert_entry_preserves_original_notification_timestamp_for_history_chronology() {
    let mut list = support::make_list();
    let mut notification = support::notification(9, "Terminal");
    notification.received_at_unix_seconds = 1_700_000_123;

    list.insert_entry(notification, false);

    assert_eq!(
        list.entries
            .get(&9)
            .expect("history entry should be stored")
            .received_at_ms,
        1_700_000_123_000
    );
}

#[test]
fn now_millis_tracks_current_unix_time() {
    let system_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_millis();
    let system_ms = i64::try_from(system_ms).expect("current millis should fit i64");

    let actual = super::now_millis();

    assert!(actual > 1_700_000_000_000);
    assert!((actual - system_ms).abs() < 5_000);
}
