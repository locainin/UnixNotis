use std::collections::{HashMap, VecDeque};
use std::rc::Rc;

use super::remove_from_group_bucket;
use crate::ui::list::test_support as support;

#[test]
fn remove_from_group_bucket_removes_only_requested_id() {
    let key = Rc::<str>::from("terminal");
    let mut map = HashMap::from([(key.clone(), VecDeque::from([3, 2, 1]))]);

    remove_from_group_bucket(&mut map, &key, 2);

    assert_eq!(map.get(&key), Some(&VecDeque::from([3, 1])));
}

#[test]
fn remove_from_group_bucket_drops_empty_bucket() {
    let key = Rc::<str>::from("terminal");
    let mut map = HashMap::from([(key.clone(), VecDeque::from([1]))]);

    remove_from_group_bucket(&mut map, &key, 1);

    assert!(!map.contains_key(&key));
}

#[test]
fn remove_from_group_bucket_ignores_unknown_group() {
    let key = Rc::<str>::from("terminal");
    let other = Rc::<str>::from("browser");
    let mut map = HashMap::from([(key.clone(), VecDeque::from([1]))]);

    remove_from_group_bucket(&mut map, &other, 1);

    assert_eq!(map.get(&key), Some(&VecDeque::from([1])));
}

#[gtk::test]
fn clear_group_indices_drops_all_parallel_group_caches() {
    let mut list = support::make_list();
    let key = Rc::<str>::from("terminal");
    list.group_active_index
        .insert(key.clone(), VecDeque::from([3, 2]));
    list.group_history_index
        .insert(key.clone(), VecDeque::from([1]));
    list.grouped_cache.insert(key, vec![3, 2, 1]);

    list.clear_group_indices();

    assert!(list.group_active_index.is_empty());
    assert!(list.group_history_index.is_empty());
    assert!(list.grouped_cache.is_empty());
}

#[gtk::test]
fn index_insert_front_deduplicates_and_syncs_group_cache() {
    let mut list = support::make_list();
    let key = Rc::<str>::from("terminal");

    list.index_insert_front(&key, 1, true);
    list.index_insert_front(&key, 2, true);
    list.index_insert_front(&key, 1, true);
    list.index_insert_front(&key, 9, false);

    assert_eq!(list.group_active_index[&key], VecDeque::from([1, 2]));
    assert_eq!(list.group_history_index[&key], VecDeque::from([9]));
    assert_eq!(list.grouped_cache[&key], vec![1, 2, 9]);
}

#[gtk::test]
fn index_remove_updates_bucket_and_cache() {
    let mut list = support::make_list();
    let key = Rc::<str>::from("terminal");
    list.index_insert_front(&key, 1, true);
    list.index_insert_front(&key, 2, true);
    list.index_insert_front(&key, 9, false);

    list.index_remove(&key, 1, true);

    assert_eq!(list.group_active_index[&key], VecDeque::from([2]));
    assert_eq!(list.grouped_cache[&key], vec![2, 9]);

    list.index_remove(&key, 2, true);
    list.index_remove(&key, 9, false);

    assert!(!list.group_active_index.contains_key(&key));
    assert!(!list.group_history_index.contains_key(&key));
    assert!(!list.grouped_cache.contains_key(&key));
}

#[gtk::test]
fn index_move_to_front_reorders_without_duplicates() {
    let mut list = support::make_list();
    let key = Rc::<str>::from("terminal");
    list.index_insert_front(&key, 1, true);
    list.index_insert_front(&key, 2, true);
    list.index_insert_front(&key, 3, true);

    list.index_move_to_front(&key, 1, true);
    list.index_move_to_front(&key, 1, true);

    assert_eq!(list.group_active_index[&key], VecDeque::from([1, 3, 2]));
    assert_eq!(list.grouped_cache[&key], vec![1, 3, 2]);
}

#[gtk::test]
fn rebuild_group_index_for_key_uses_current_orders_and_entries() {
    let mut list = support::make_list();
    list.seed(
        vec![
            support::notification(1, "Terminal"),
            support::notification(2, "Browser"),
            support::notification(3, "Terminal"),
        ],
        vec![support::notification(4, "Terminal")],
    );
    let key = list.entries.get(&3).expect("entry").app_key.clone();
    list.clear_group_indices();

    list.rebuild_group_index_for_key(&key);

    assert_eq!(list.group_active_index[&key], VecDeque::from([3, 1]));
    assert_eq!(list.group_history_index[&key], VecDeque::from([4]));
    assert_eq!(list.grouped_cache[&key], vec![3, 1, 4]);
}
