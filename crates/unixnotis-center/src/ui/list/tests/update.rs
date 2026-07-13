use gio::prelude::ListModelExt;
use gtk::prelude::WidgetExt;
use std::rc::Rc;

use crate::ui::list::item::RowData;
use crate::ui::list::test_support as support;
use crate::ui::list::types::{GroupRange, RowKey};

use super::{
    has_pending_items, intern_key_is_live, merge_adjacent_ranges, range_count_mismatch,
    should_keep_group, should_rebuild_from_scratch,
};

#[gtk::test]
fn request_rebuild_marks_list_dirty() {
    let mut list = support::make_list();
    assert!(!list.needs_rebuild());

    list.request_rebuild();

    assert!(list.needs_rebuild());
}

#[test]
fn rebuild_from_scratch_policy_covers_empty_store_and_missing_ranges() {
    assert!(should_rebuild_from_scratch(0, 0));
    assert!(should_rebuild_from_scratch(0, 2));
    assert!(should_rebuild_from_scratch(3, 0));
    assert!(!should_rebuild_from_scratch(3, 2));
}

#[test]
fn pending_item_policy_and_range_count_mismatch_use_counts() {
    assert!(!has_pending_items(0));
    assert!(has_pending_items(1));
    assert!(!range_count_mismatch(2, 2));
    assert!(range_count_mismatch(1, 2));
    assert!(range_count_mismatch(3, 2));
}

#[gtk::test]
fn group_ids_are_visible_respects_empty_ids_and_active_filter() {
    let mut list = support::make_list();
    list.seed(
        vec![
            support::notification(1, "Terminal"),
            support::notification(2, "Browser"),
        ],
        Vec::new(),
    );
    let terminal = list.entries.get(&1).expect("terminal").app_key.clone();
    let browser = list.entries.get(&2).expect("browser").app_key.clone();
    let terminal_ids = list
        .grouped_cache
        .get(&terminal)
        .expect("terminal ids")
        .clone();
    let browser_ids = list
        .grouped_cache
        .get(&browser)
        .expect("browser ids")
        .clone();

    assert!(!list.group_ids_are_visible(&[]));
    assert!(list.group_ids_are_visible(&terminal_ids));

    assert!(list.set_filter_query("browser"));
    assert!(!list.group_ids_are_visible(&terminal_ids));
    assert!(list.group_ids_are_visible(&browser_ids));
}

#[test]
fn intern_key_liveness_requires_a_reference_outside_the_intern_set() {
    let stale = Rc::<str>::from("stale");
    assert!(!intern_key_is_live(&stale));

    let live = Rc::<str>::from("live");
    let _external = live.clone();
    assert!(intern_key_is_live(&live));
}

#[test]
fn keep_group_policy_requires_clean_group_and_matching_span() {
    let key = Rc::<str>::from("terminal");
    let mut dirty = std::collections::HashSet::new();

    assert!(should_keep_group(&dirty, &key, 2, 2));
    assert!(!should_keep_group(&dirty, &key, 2, 3));

    dirty.insert(key.clone());
    assert!(!should_keep_group(&dirty, &key, 2, 2));
}

#[test]
fn merge_adjacent_ranges_combines_only_touching_ranges() {
    let merged = merge_adjacent_ranges(vec![
        GroupRange { start: 6, len: 1 },
        GroupRange { start: 0, len: 3 },
        GroupRange { start: 3, len: 2 },
        GroupRange { start: 8, len: 1 },
    ]);

    assert_eq!(merged.len(), 3);
    assert_eq!(merged[0].start, 0);
    assert_eq!(merged[0].len, 5);
    assert_eq!(merged[1].start, 6);
    assert_eq!(merged[1].len, 1);
    assert_eq!(merged[2].start, 8);
    assert_eq!(merged[2].len, 1);
}

#[gtk::test]
fn flush_rebuild_noops_when_list_is_clean() {
    let mut list = support::make_list();
    list.flush_rebuild();

    assert!(!list.needs_rebuild());
    assert_eq!(list.store.n_items(), 0);
}

#[gtk::test]
fn flush_rebuild_builds_seeded_rows_and_hides_empty_overlay() {
    let mut list = support::make_list();
    list.seed(vec![support::notification(1, "Terminal")], Vec::new());

    list.flush_rebuild();

    assert!(!list.needs_rebuild());
    assert_eq!(list.store.n_items(), 2);
    assert!(!list.empty_overlay.get_visible());
}

#[gtk::test]
fn flush_rebuild_filters_existing_list_with_minimal_middle_splice() {
    let mut list = support::make_list();
    list.seed(
        vec![
            support::notification(1, "Terminal"),
            support::notification(2, "Browser"),
        ],
        Vec::new(),
    );
    list.flush_rebuild();
    let browser = list.entries.get(&2).expect("browser").app_key.clone();
    assert_eq!(list.store.n_items(), 4);

    assert!(list.set_filter_query("browser"));
    list.flush_rebuild();

    assert_eq!(list.store.n_items(), 2);
    assert_eq!(
        list.current_keys,
        vec![
            RowKey::GroupHeader {
                group: browser.clone()
            },
            RowKey::Notification { id: 2 },
        ]
    );
    assert_eq!(list.group_ranges[&browser].start, 0);
    assert_eq!(list.group_ranges[&browser].len, 2);
}

#[gtk::test]
fn flush_rebuild_rebuilds_from_nonempty_store_when_ranges_are_missing() {
    let mut list = support::make_list();
    list.seed(
        vec![
            support::notification(1, "Terminal"),
            support::notification(2, "Browser"),
            support::notification(3, "Editor"),
        ],
        Vec::new(),
    );
    list.flush_rebuild();
    let terminal = list.entries.get(&1).expect("terminal").app_key.clone();
    let browser = list.entries.get(&2).expect("browser").app_key.clone();
    let editor = list.entries.get(&3).expect("editor").app_key.clone();
    list.interned.insert(Rc::from("stale"));

    list.remove_entry(2);
    list.group_ranges.clear();
    list.request_rebuild();
    list.flush_rebuild();

    assert_eq!(list.store.n_items(), 4);
    assert_eq!(
        list.current_keys,
        vec![
            RowKey::GroupHeader {
                group: editor.clone()
            },
            RowKey::Notification { id: 3 },
            RowKey::GroupHeader {
                group: terminal.clone()
            },
            RowKey::Notification { id: 1 },
        ]
    );
    assert!(!list.group_ranges.contains_key(&browser));
    assert_eq!(list.group_ranges[&editor].start, 0);
    assert_eq!(list.group_ranges[&terminal].start, 2);
    assert!(!list.interned.iter().any(|key| key.as_ref() == "stale"));
}

#[gtk::test]
fn flush_rebuild_applies_dirty_group_span_changes_incrementally() {
    let mut list = support::make_list();
    list.seed(
        vec![
            support::notification(1, "Terminal"),
            support::notification(2, "Terminal"),
            support::notification(3, "Browser"),
        ],
        Vec::new(),
    );
    list.flush_rebuild();
    let terminal = list.entries.get(&2).expect("terminal").app_key.clone();
    let browser = list.entries.get(&3).expect("browser").app_key.clone();

    list.toggle_group(terminal.as_ref());
    list.flush_rebuild();

    assert_eq!(list.store.n_items(), 5);
    assert_eq!(
        list.current_keys,
        vec![
            RowKey::GroupHeader {
                group: browser.clone()
            },
            RowKey::Notification { id: 3 },
            RowKey::GroupHeader {
                group: terminal.clone()
            },
            RowKey::Notification { id: 2 },
            RowKey::Notification { id: 1 },
        ]
    );
    assert_eq!(list.group_ranges[&browser].start, 0);
    assert_eq!(list.group_ranges[&browser].len, 2);
    assert_eq!(list.group_ranges[&terminal].start, 2);
    assert_eq!(list.group_ranges[&terminal].len, 3);
}

#[gtk::test]
fn flush_rebuild_refreshes_dirty_group_even_when_span_is_stable() {
    let mut list = support::make_list();
    list.seed(vec![support::notification(1, "Terminal")], Vec::new());
    list.flush_rebuild();
    let terminal = list.entries.get(&1).expect("terminal").app_key.clone();
    let view = list.entries.get(&1).expect("terminal").view.clone();
    let header = list.group_headers.get(&terminal).expect("header").clone();
    header.update(RowData::group_header(terminal.clone(), 99, false, view));

    list.dirty_groups.insert(terminal.clone());
    list.request_rebuild();
    list.flush_rebuild();

    assert_eq!(list.store.n_items(), 2);
    assert_eq!(header.data().count, 1);
    assert_eq!(list.group_ranges[&terminal].start, 0);
    assert_eq!(list.group_ranges[&terminal].len, 2);
}

#[gtk::test]
fn flush_rebuild_places_multiple_pending_dirty_groups_before_kept_group() {
    let mut list = support::make_list();
    list.seed(vec![support::notification(1, "Terminal")], Vec::new());
    list.flush_rebuild();

    list.add_or_update(support::notification(2, "Browser"), true);
    list.add_or_update(support::notification(3, "Editor"), true);
    list.flush_rebuild();

    let terminal = list.entries.get(&1).expect("terminal").app_key.clone();
    let browser = list.entries.get(&2).expect("browser").app_key.clone();
    let editor = list.entries.get(&3).expect("editor").app_key.clone();
    assert_eq!(list.store.n_items(), 6);
    assert_eq!(list.group_ranges[&editor].start, 0);
    assert_eq!(list.group_ranges[&browser].start, 2);
    assert_eq!(list.group_ranges[&terminal].start, 4);
}

#[gtk::test]
fn flush_rebuild_removes_empty_dirty_group_and_keeps_following_ranges_valid() {
    let mut list = support::make_list();
    list.seed(
        vec![
            support::notification(1, "Terminal"),
            support::notification(2, "Browser"),
        ],
        Vec::new(),
    );
    list.flush_rebuild();
    let terminal = list.entries.get(&1).expect("terminal").app_key.clone();
    let browser = list.entries.get(&2).expect("browser").app_key.clone();

    list.remove_entry(2);
    list.dirty_groups.insert(browser.clone());
    list.request_rebuild();
    list.flush_rebuild();

    assert_eq!(list.store.n_items(), 2);
    assert_eq!(
        list.current_keys,
        vec![
            RowKey::GroupHeader {
                group: terminal.clone()
            },
            RowKey::Notification { id: 1 },
        ]
    );
    assert!(!list.group_ranges.contains_key(&browser));
    assert_eq!(list.group_ranges[&terminal].start, 0);
    assert_eq!(list.group_ranges[&terminal].len, 2);
}

#[gtk::test]
fn flush_rebuild_restores_missing_range_with_full_rebuild_fallback() {
    let mut list = support::make_list();
    list.seed(
        vec![
            support::notification(1, "Terminal"),
            support::notification(2, "Browser"),
        ],
        Vec::new(),
    );
    list.flush_rebuild();
    let terminal = list.entries.get(&1).expect("terminal").app_key.clone();
    let browser = list.entries.get(&2).expect("browser").app_key.clone();
    list.group_ranges.remove(&terminal);

    list.request_rebuild();
    list.flush_rebuild();

    assert_eq!(list.store.n_items(), 4);
    assert_eq!(list.group_ranges[&browser].start, 0);
    assert_eq!(list.group_ranges[&terminal].start, 2);
}

#[gtk::test]
fn flush_rebuild_restores_store_length_with_full_rebuild_fallback() {
    let mut list = support::make_list();
    list.seed(
        vec![
            support::notification(1, "Terminal"),
            support::notification(2, "Browser"),
        ],
        Vec::new(),
    );
    list.flush_rebuild();
    list.store.remove(0);

    list.request_rebuild();
    list.flush_rebuild();

    assert_eq!(list.store.n_items(), 4);
    assert_eq!(list.current_keys.len(), 4);
}

#[gtk::test]
fn flush_rebuild_batches_new_dirty_groups_before_kept_groups() {
    let mut list = support::make_list();
    list.seed(vec![support::notification(1, "Terminal")], Vec::new());
    list.flush_rebuild();

    list.add_or_update(support::notification(2, "Browser"), true);
    list.flush_rebuild();

    let browser = list.entries.get(&2).expect("browser").app_key.clone();
    let terminal = list.entries.get(&1).expect("terminal").app_key.clone();
    assert_eq!(list.store.n_items(), 4);
    assert_eq!(
        list.current_keys,
        vec![
            RowKey::GroupHeader {
                group: browser.clone()
            },
            RowKey::Notification { id: 2 },
            RowKey::GroupHeader {
                group: terminal.clone()
            },
            RowKey::Notification { id: 1 },
        ]
    );
    assert_eq!(list.group_ranges[&browser].start, 0);
    assert_eq!(list.group_ranges[&terminal].start, 2);
}
