use unixnotis_core::{Action, NotificationImage};

use super::*;
use crate::ui::notifications::test_support as support;

fn make_view(is_transient: bool) -> NotificationView {
    NotificationView {
        id: 7,
        generation: 7,
        app_name: "Test".to_string(),
        attribution: unixnotis_core::NotificationAttribution {
            display_name: "Test".to_string(),
            group_key: "test:Test".to_string(),
            ..unixnotis_core::NotificationAttribution::default()
        },
        summary: "summary".to_string(),
        body: "body".to_string(),
        actions: vec![Action {
            key: "default".to_string(),
            label: "Open".to_string(),
        }],
        inline_reply: unixnotis_core::InlineReply::default(),
        inline_reply_policy: unixnotis_core::InlineReplyPolicy::Allow,
        urgency: 1,
        category: String::new(),
        is_transient,
        received_at_unix_seconds: 0,
        image: NotificationImage::default(),
    }
}

fn view(id: u32, app_name: &str, is_transient: bool) -> NotificationView {
    NotificationView {
        id,
        generation: u64::from(id),
        app_name: app_name.to_string(),
        attribution: unixnotis_core::NotificationAttribution {
            display_name: app_name.to_string(),
            group_key: format!("test:{app_name}"),
            ..unixnotis_core::NotificationAttribution::default()
        },
        summary: format!("summary {id}"),
        body: format!("body {id}"),
        actions: Vec::new(),
        inline_reply: unixnotis_core::InlineReply::default(),
        inline_reply_policy: unixnotis_core::InlineReplyPolicy::Allow,
        urgency: 1,
        category: String::new(),
        is_transient,
        received_at_unix_seconds: 0,
        image: NotificationImage::default(),
    }
}

fn notification_key(id: u32) -> NotificationKey {
    NotificationKey {
        id,
        generation: u64::from(id),
    }
}

#[test]
fn active_move_policy_covers_history_new_and_non_front_rows() {
    assert!(should_move_active_to_front(true, false, false));
    assert!(should_move_active_to_front(true, true, true));
    assert!(should_move_active_to_front(false, false, false));
    assert!(should_move_active_to_front(false, true, false));
    assert!(!should_move_active_to_front(false, true, true));
}

#[test]
fn collapsed_group_stacked_policy_requires_collapsed_group_with_multiple_rows() {
    assert!(!collapsed_group_is_stacked(false, 0));
    assert!(!collapsed_group_is_stacked(false, 1));
    assert!(collapsed_group_is_stacked(false, 2));
    assert!(!collapsed_group_is_stacked(true, 2));
}

#[test]
fn transient_rows_follow_config_when_closed() {
    assert!(!should_archive_entry(
        &make_view(true),
        CloseReason::Expired,
        false
    ));
    assert!(should_archive_entry(
        &make_view(true),
        CloseReason::Expired,
        true
    ));
}

#[test]
fn user_dismiss_never_archives_locally() {
    assert!(!should_archive_entry(
        &make_view(false),
        CloseReason::DismissedByUser,
        true
    ));
    assert!(!should_archive_entry(
        &make_view(true),
        CloseReason::DismissedByUser,
        true
    ));
}

#[gtk::test]
fn add_or_update_new_active_notification_updates_storage_and_requests_rebuild() {
    let mut list = support::make_list();

    list.add_or_update(view(1, "Terminal", false), true);

    let key = list.entries.get(&1).expect("entry").app_key.clone();
    assert_eq!(
        list.active_order.iter().copied().collect::<Vec<_>>(),
        vec![1]
    );
    assert!(list.history_order.is_empty());
    assert_eq!(
        list.group_active_index[&key]
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        vec![1]
    );
    assert_eq!(list.grouped_cache[&key], vec![1]);
    assert!(list.dirty_groups.contains(&key));
    assert!(list.needs_rebuild());
}

#[gtk::test]
fn add_or_update_existing_front_active_row_uses_in_place_fast_path() {
    let mut list = support::make_list();
    list.seed(vec![view(1, "Terminal", false)], Vec::new());
    list.flush_rebuild();
    let key = list.entries.get(&1).expect("entry").app_key.clone();
    let mut updated = view(1, "Terminal", false);
    updated.summary = "changed".to_string();

    list.add_or_update(updated, true);

    assert!(!list.needs_rebuild());
    assert!(list.dirty_groups.is_empty());
    assert_eq!(list.grouped_cache[&key], vec![1]);
    let row = list.entries.get(&1).expect("entry").item.data();
    assert_eq!(
        row.notification.expect("notification").summary.as_str(),
        "changed"
    );
}

#[gtk::test]
fn add_or_update_non_front_history_row_does_not_replace_group_header_sample() {
    let mut list = support::make_list();
    list.seed(
        Vec::new(),
        vec![view(1, "Terminal", false), view(2, "Terminal", false)],
    );
    list.flush_rebuild();
    let key = list.entries.get(&2).expect("entry").app_key.clone();
    let header = list.group_headers.get(&key).expect("header").clone();
    assert_eq!(
        header
            .data()
            .notification
            .as_ref()
            .expect("header sample")
            .id,
        2
    );

    let mut updated = view(1, "Terminal", false);
    updated.summary = "older changed".to_string();
    list.add_or_update(updated, false);

    assert!(!list.needs_rebuild());
    assert_eq!(
        header
            .data()
            .notification
            .as_ref()
            .expect("header sample")
            .id,
        2
    );
}

#[gtk::test]
fn add_or_update_existing_active_row_moves_non_front_id_to_front() {
    let mut list = support::make_list();
    list.seed(
        vec![view(1, "Terminal", false), view(2, "Browser", false)],
        Vec::new(),
    );
    list.flush_rebuild();

    list.add_or_update(view(1, "Terminal", false), true);

    let key = list.entries.get(&1).expect("entry").app_key.clone();
    assert_eq!(
        list.active_order.iter().copied().collect::<Vec<_>>(),
        vec![1, 2]
    );
    assert_eq!(list.group_active_index[&key].front().copied(), Some(1));
    assert!(list.dirty_groups.contains(&key));
    assert!(list.needs_rebuild());
}

#[gtk::test]
fn add_or_update_history_row_promoted_to_active_moves_between_indices() {
    let mut list = support::make_list();
    list.seed(Vec::new(), vec![view(1, "Terminal", false)]);
    list.flush_rebuild();

    list.add_or_update(view(1, "Terminal", false), true);

    let key = list.entries.get(&1).expect("entry").app_key.clone();
    assert_eq!(
        list.active_order.iter().copied().collect::<Vec<_>>(),
        vec![1]
    );
    assert!(list.history_order.is_empty());
    assert_eq!(
        list.group_active_index[&key]
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        vec![1]
    );
    assert!(!list.group_history_index.contains_key(&key));
}

#[gtk::test]
fn add_or_update_app_name_change_reindexes_old_and_new_groups() {
    let mut list = support::make_list();
    list.seed(vec![view(1, "Terminal", false)], Vec::new());
    list.flush_rebuild();
    let old_key = list.entries.get(&1).expect("entry").app_key.clone();

    list.add_or_update(view(1, "Browser", false), true);

    let new_key = list.entries.get(&1).expect("entry").app_key.clone();
    assert_ne!(old_key.as_ref(), new_key.as_ref());
    assert!(!list.grouped_cache.contains_key(&old_key));
    assert_eq!(list.grouped_cache[&new_key], vec![1]);
    assert!(list.dirty_groups.contains(&old_key));
    assert!(list.dirty_groups.contains(&new_key));
}

#[gtk::test]
fn group_span_matches_visible_shape_detects_missing_or_stale_ranges() {
    let mut list = support::make_list();
    list.seed(
        vec![view(1, "Terminal", false), view(2, "Terminal", false)],
        Vec::new(),
    );
    list.flush_rebuild();
    let key = list.entries.get(&1).expect("entry").app_key.clone();

    assert!(list.group_span_matches_visible_shape(&key));

    list.group_ranges.get_mut(&key).expect("range").len = 99;
    assert!(!list.group_span_matches_visible_shape(&key));

    list.group_ranges.remove(&key);
    assert!(!list.group_span_matches_visible_shape(&key));
}

#[gtk::test]
fn mark_closed_dismissed_row_removes_entry_and_marks_group_dirty() {
    let mut list = support::make_list();
    list.seed(vec![view(1, "Terminal", false)], Vec::new());
    list.flush_rebuild();
    let key = list.entries.get(&1).expect("entry").app_key.clone();

    list.mark_closed(notification_key(1), CloseReason::DismissedByUser);

    assert!(!list.entries.contains_key(&1));
    assert!(list.active_order.is_empty());
    assert!(!list.grouped_cache.contains_key(&key));
    assert!(list.dirty_groups.contains(&key));
    assert!(list.needs_rebuild());
}

#[gtk::test]
fn mark_closed_expired_row_archives_to_history_when_policy_allows_it() {
    let mut list = support::make_list();
    list.seed(vec![view(1, "Terminal", false)], Vec::new());
    list.flush_rebuild();
    let key = list.entries.get(&1).expect("entry").app_key.clone();

    list.mark_closed(notification_key(1), CloseReason::Expired);

    assert!(list.active_order.is_empty());
    assert_eq!(
        list.history_order.iter().copied().collect::<Vec<_>>(),
        vec![1]
    );
    assert!(!list.group_active_index.contains_key(&key));
    assert_eq!(
        list.group_history_index[&key]
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        vec![1]
    );
    assert_eq!(list.grouped_cache[&key], vec![1]);
    assert!(list.dirty_groups.contains(&key));
}

#[gtk::test]
fn mark_closed_archived_row_does_not_duplicate_existing_history_id() {
    let mut list = support::make_list();
    list.seed(vec![view(1, "Terminal", false)], Vec::new());
    list.flush_rebuild();

    list.mark_closed(notification_key(1), CloseReason::Expired);
    list.needs_rebuild = false;
    list.mark_closed(notification_key(1), CloseReason::Expired);

    assert_eq!(
        list.history_order.iter().copied().collect::<Vec<_>>(),
        vec![1]
    );
}

#[gtk::test]
fn reordered_update_cannot_replace_a_newer_row_generation() {
    let mut list = support::make_list();
    let mut newest = view(1, "Terminal", false);
    newest.generation = 3;
    list.seed(vec![newest], Vec::new());
    let mut stale = view(1, "Terminal", false);
    stale.generation = 2;
    stale.summary = "stale payload".to_string();

    list.add_or_update(stale, true);

    let current = &list.entries.get(&1).expect("current row").view;
    assert_eq!(current.generation, 3);
    assert_ne!(current.summary, "stale payload");
}

#[gtk::test]
fn reordered_close_cannot_remove_or_archive_a_newer_row_generation() {
    let mut list = support::make_list();
    let mut replacement = view(1, "Terminal", false);
    replacement.generation = 3;
    list.seed(vec![replacement], Vec::new());

    list.mark_closed(
        NotificationKey {
            id: 1,
            generation: 2,
        },
        CloseReason::Expired,
    );

    assert!(list.entries.get(&1).expect("replacement row").is_active);
    assert_eq!(list.active_order.iter().copied().collect::<Vec<_>>(), [1]);
    assert!(list.history_order.is_empty());
}
