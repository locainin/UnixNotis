use unixnotis_core::{Action, NotificationImage};

use super::*;

fn make_view(is_transient: bool) -> NotificationView {
    NotificationView {
        id: 7,
        app_name: "Test".to_string(),
        summary: "summary".to_string(),
        body: "body".to_string(),
        actions: vec![Action {
            key: "default".to_string(),
            label: "Open".to_string(),
        }],
        urgency: 1,
        is_transient,
        image: NotificationImage::default(),
    }
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
