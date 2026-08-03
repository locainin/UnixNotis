//! Shared fixtures for notification-row tests

use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};

use gtk::prelude::*;
use unixnotis_core::{NotificationImage, NotificationView, Urgency};

use crate::ui::notifications::item::{RowData, RowPresentation};
use crate::ui::notifications::test_support as support;

use super::build::build_notification_row;
use super::state::NotificationRowWidgets;

pub(super) fn sample_notification() -> NotificationView {
    NotificationView {
        id: 1,
        generation: 1,
        app_name: "demo".to_string(),
        attribution: unixnotis_core::NotificationAttribution::verified(
            "demo",
            "demo",
            "org.example.Demo",
            "demo",
            unixnotis_core::AttributionReason::ExactSystemExecutable,
            "authenticated test fixture",
            "test:demo".to_string(),
        ),
        summary: "summary".to_string(),
        body: "body".to_string(),
        actions: Vec::new(),
        inline_reply: unixnotis_core::InlineReply::default(),
        inline_reply_policy: unixnotis_core::InlineReplyPolicy::Allow,
        urgency: Urgency::Normal as u8,
        category: String::new(),
        is_transient: false,
        received_at_unix_seconds: 0,
        image: NotificationImage::default(),
        popup_decision: unixnotis_core::PopupDecisionRecord::default(),
        popup_hide_after_ms: 0,
    }
}

pub(super) fn notification_row() -> (gtk::Box, NotificationRowWidgets) {
    support::init_gtk();
    let (command_tx, _rx) = tokio::sync::mpsc::channel(4);
    build_notification_row(command_tx)
}

pub(super) fn notification_row_with_receiver() -> (
    gtk::Box,
    NotificationRowWidgets,
    tokio::sync::mpsc::Receiver<crate::control::UiCommand>,
) {
    support::init_gtk();
    let (command_tx, command_rx) = tokio::sync::mpsc::channel(4);
    let (root, row) = build_notification_row(command_tx);
    (root, row, command_rx)
}

pub(super) struct RowFlags {
    pub(super) is_active: bool,
    pub(super) collapsed_group_preview: bool,
    pub(super) show_metadata: bool,
    pub(super) show_thumbnail: bool,
    pub(super) show_avatar: bool,
    pub(super) reduced_motion: bool,
    pub(super) metadata: Option<unixnotis_core::NotificationMetadataConfig>,
    pub(super) card_corners: unixnotis_core::CutCorners,
}

impl Default for RowFlags {
    fn default() -> Self {
        Self {
            is_active: false,
            collapsed_group_preview: false,
            show_metadata: false,
            show_thumbnail: false,
            show_avatar: true,
            reduced_motion: false,
            metadata: None,
            card_corners: unixnotis_core::CutCorners::default(),
        }
    }
}

pub(super) fn row_data(notification: Rc<NotificationView>, flags: RowFlags) -> RowData {
    RowData::notification(
        Rc::from(notification.app_name.to_ascii_lowercase()),
        notification,
        flags.collapsed_group_preview,
        u8::from(flags.collapsed_group_preview),
        false,
        flags.is_active,
        RowPresentation {
            received_at_ms: current_millis(),
            show_metadata: flags.show_metadata,
            show_thumbnail: flags.show_thumbnail,
            show_avatar: flags.show_avatar,
            reduced_motion: flags.reduced_motion,
            metadata: Rc::new(flags.metadata.unwrap_or_default()),
            card_corners: flags.card_corners,
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
