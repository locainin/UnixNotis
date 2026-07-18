use std::collections::HashSet;
use std::rc::Rc;

use super::{FilterQuery, GroupRange, RowKey};

#[test]
fn row_key_hash_and_equality_keep_group_and_notification_distinct() {
    let group = RowKey::GroupHeader {
        group: Rc::from("terminal"),
    };
    let notification = RowKey::Notification { id: 1 };
    let mut keys = HashSet::new();

    keys.insert(group.clone());
    keys.insert(notification.clone());
    keys.insert(group);
    keys.insert(notification);

    assert_eq!(keys.len(), 2);
}

#[test]
fn filter_query_equality_includes_ascii_mode() {
    let ascii = FilterQuery {
        text: "term".into(),
        ascii_only: true,
    };
    let unicode = FilterQuery {
        text: "term".into(),
        ascii_only: false,
    };

    assert_ne!(ascii, unicode);
}

#[test]
fn group_range_copy_preserves_span() {
    let range = GroupRange { start: 4, len: 2 };
    let copied = range;

    assert_eq!(copied.start, 4);
    assert_eq!(copied.len, 2);
}
