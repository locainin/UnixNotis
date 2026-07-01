//! Shared fixtures for notification-row tests

use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};

use gtk::prelude::*;
use unixnotis_core::{NotificationImage, NotificationView, Urgency};

use crate::ui::list::item::{RowData, RowPresentation};
use crate::ui::list::test_support as support;

use super::build::build_notification_row;
use super::state::NotificationRowWidgets;

pub(super) fn sample_notification() -> NotificationView {
    NotificationView {
        id: 1,
        app_name: "demo".to_string(),
        summary: "summary".to_string(),
        body: "body".to_string(),
        actions: Vec::new(),
        urgency: Urgency::Normal as u8,
        is_transient: false,
        image: NotificationImage::default(),
    }
}

pub(super) fn notification_row() -> (gtk::Box, NotificationRowWidgets) {
    support::init_gtk();
    let (command_tx, _rx) = tokio::sync::mpsc::channel(4);
    build_notification_row(command_tx)
}

pub(super) fn row_data(
    notification: Rc<NotificationView>,
    is_active: bool,
    stacked: bool,
    stack_depth: u8,
    show_metadata: bool,
    show_thumbnail: bool,
) -> RowData {
    RowData::notification(
        Rc::from(notification.app_name.to_ascii_lowercase()),
        notification,
        stacked,
        stack_depth,
        false,
        is_active,
        RowPresentation {
            received_at_ms: current_millis(),
            show_metadata,
            show_thumbnail,
        },
    )
}

pub(super) fn current_millis() -> i64 {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after epoch")
        .as_millis();
    i64::try_from(millis).expect("current millis should fit i64")
}

pub(super) fn child_count(container: &gtk::Box) -> usize {
    let mut count = 0;
    let mut child = container.first_child();
    while let Some(widget) = child {
        count += 1;
        child = widget.next_sibling();
    }
    count
}
