use std::rc::Rc;

use gio::prelude::ListModelExt;

use super::common_prefix_suffix;
use crate::ui::notifications::item::{RowData, RowItem};
use crate::ui::notifications::model::types::{GroupRange, RowKey};
use crate::ui::notifications::test_support as support;

#[test]
fn common_prefix_suffix_finds_stable_edges() {
    let group = Rc::<str>::from("terminal");
    let current = vec![
        RowKey::GroupHeader {
            group: group.clone(),
        },
        RowKey::Notification { id: 1 },
        RowKey::Notification { id: 2 },
        RowKey::Notification { id: 3 },
    ];
    let next = vec![
        RowKey::GroupHeader { group },
        RowKey::Notification { id: 9 },
        RowKey::Notification { id: 2 },
        RowKey::Notification { id: 3 },
    ];

    assert_eq!(common_prefix_suffix(&current, &next), (1, 2));
}

#[test]
fn common_prefix_suffix_handles_empty_inputs() {
    assert_eq!(common_prefix_suffix(&[], &[]), (0, 0));
    assert_eq!(
        common_prefix_suffix(&[], &[RowKey::Notification { id: 1 }]),
        (0, 0)
    );
}

#[test]
fn common_prefix_suffix_does_not_overlap_when_lists_are_equal() {
    let keys = vec![
        RowKey::Notification { id: 1 },
        RowKey::Notification { id: 2 },
    ];

    assert_eq!(common_prefix_suffix(&keys, &keys), (2, 0));
}

#[gtk::test]
fn build_group_block_collapses_group_to_header_and_top_notification() {
    let mut list = support::make_list();
    list.seed(
        vec![
            support::notification(1, "Terminal"),
            support::notification(2, "Terminal"),
            support::notification(3, "Terminal"),
        ],
        Vec::new(),
    );
    let key = list.entries.get(&3).expect("entry").app_key.clone();
    let ids = list.grouped_cache.get(&key).expect("group ids").clone();

    let (items, keys) = list.build_group_block(&key, &ids);

    assert_eq!(items.len(), 2);
    assert_eq!(
        keys,
        vec![
            RowKey::GroupHeader { group: key.clone() },
            RowKey::Notification { id: 3 }
        ]
    );
    let header = items[0].data();
    assert_eq!(header.count, 3);
    assert!(!header.expanded);
    let visible = items[1].data();
    assert!(visible.collapsed_group_preview);
    assert!(!visible.expanded);
    assert!(visible.group_first);
    assert!(visible.group_last);
}

#[gtk::test]
fn build_group_block_keeps_single_notification_outside_collapsed_group_preview() {
    let mut list = support::make_list();
    list.seed(vec![support::notification(1, "Terminal")], Vec::new());
    let key = list.entries.get(&1).expect("entry").app_key.clone();
    let ids = list.grouped_cache.get(&key).expect("group ids").clone();

    let (items, _keys) = list.build_group_block(&key, &ids);

    assert_eq!(items.len(), 1);
    let visible = items[0].data();
    assert!(!visible.collapsed_group_preview);
    assert!(!visible.group_first);
    assert!(!visible.group_last);
}

#[gtk::test]
fn build_group_block_expands_group_to_all_notifications() {
    let mut list = support::make_list();
    list.seed(
        vec![
            support::notification(1, "Terminal"),
            support::notification(2, "Terminal"),
            support::notification(3, "Terminal"),
        ],
        Vec::new(),
    );
    let key = list.entries.get(&3).expect("entry").app_key.clone();
    list.group_expanded.insert(key.clone(), true);
    let ids = list.grouped_cache.get(&key).expect("group ids").clone();

    let (items, keys) = list.build_group_block(&key, &ids);

    assert_eq!(list.group_block_len(&key, &ids), 4);
    assert_eq!(items.len(), 4);
    assert_eq!(
        keys,
        vec![
            RowKey::GroupHeader { group: key.clone() },
            RowKey::Notification { id: 3 },
            RowKey::Notification { id: 2 },
            RowKey::Notification { id: 1 },
        ]
    );
    for item in items.iter().skip(1) {
        let data = item.data();
        assert!(!data.collapsed_group_preview);
        assert!(data.expanded);
    }
    let first = items[1].data();
    let middle = items[2].data();
    let last = items[3].data();
    assert!(first.group_first);
    assert!(!first.group_last);
    assert!(!middle.group_first);
    assert!(!middle.group_last);
    assert!(!last.group_first);
    assert!(last.group_last);
}

#[gtk::test]
fn group_block_len_counts_header_and_visible_rows() {
    let mut list = support::make_list();
    let key = Rc::<str>::from("terminal");
    let ids = vec![1, 2, 3];

    assert_eq!(list.group_block_len(&key, &ids), 2);

    list.group_expanded.insert(key.clone(), true);
    assert_eq!(list.group_block_len(&key, &ids), 4);
}

#[gtk::test]
fn insert_and_remove_block_keep_store_keys_and_ranges_in_sync() {
    let mut list = support::make_list();
    let terminal = Rc::<str>::from("terminal");
    let browser = Rc::<str>::from("browser");
    let existing_items = vec![RowItem::new(RowData::default())];
    let existing_keys = vec![RowKey::Notification { id: 0 }];

    assert_eq!(list.insert_block(0, &existing_items, &existing_keys), 1);

    list.group_ranges
        .insert(terminal.clone(), GroupRange { start: 0, len: 1 });
    list.group_ranges
        .insert(browser.clone(), GroupRange { start: 1, len: 1 });

    let items = vec![
        RowItem::new(RowData::default()),
        RowItem::new(RowData::default()),
    ];
    let keys = vec![
        RowKey::Notification { id: 1 },
        RowKey::Notification { id: 2 },
    ];

    assert_eq!(list.insert_block(1, &items, &keys), 2);
    assert_eq!(list.store.n_items(), 3);
    assert_eq!(
        list.current_keys,
        vec![
            RowKey::Notification { id: 0 },
            RowKey::Notification { id: 1 },
            RowKey::Notification { id: 2 },
        ]
    );
    assert_eq!(list.group_ranges[&terminal].start, 0);
    assert_eq!(list.group_ranges[&browser].start, 3);

    list.remove_block(1, 1);

    assert_eq!(list.store.n_items(), 2);
    assert_eq!(
        list.current_keys,
        vec![
            RowKey::Notification { id: 0 },
            RowKey::Notification { id: 2 },
        ]
    );
    assert_eq!(list.group_ranges[&terminal].start, 0);
    assert_eq!(list.group_ranges[&browser].start, 2);
}

#[gtk::test]
fn shift_group_ranges_excludes_exact_start_for_removals() {
    let mut list = support::make_list();
    let exact = Rc::<str>::from("exact");
    let after = Rc::<str>::from("after");
    list.group_ranges
        .insert(exact.clone(), GroupRange { start: 1, len: 1 });
    list.group_ranges
        .insert(after.clone(), GroupRange { start: 2, len: 1 });

    list.shift_group_ranges(1, -1, false);

    assert_eq!(list.group_ranges[&exact].start, 1);
    assert_eq!(list.group_ranges[&after].start, 1);
}

#[gtk::test]
fn insert_block_noops_when_there_are_no_items() {
    let mut list = support::make_list();

    assert_eq!(list.insert_block(0, &[], &[]), 0);
    assert_eq!(list.store.n_items(), 0);
    assert!(list.current_keys.is_empty());
}
