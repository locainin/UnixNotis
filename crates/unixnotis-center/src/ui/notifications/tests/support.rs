use std::rc::Rc;
use std::sync::Once;

use async_channel::Sender;
use tokio::sync::mpsc;
use unixnotis_core::{NotificationImage, NotificationView};

use crate::control::{UiCommand, UiEvent};
use crate::ui::icons::IconResolver;
use crate::ui::notifications::{NotificationList, NotificationListConfig};

static GTK_INIT: Once = Once::new();

pub(super) fn init_gtk() {
    GTK_INIT.call_once(|| {
        gtk::init().expect("gtk should initialize under xvfb");
    });
}

pub(super) fn list_config() -> NotificationListConfig {
    NotificationListConfig {
        max_active: 10,
        max_entries: 10,
        transient_to_history: true,
        show_notification_metadata: false,
        notification_metadata: unixnotis_core::NotificationMetadataConfig::default(),
        notification_corners: unixnotis_core::CutCorners::default(),
        show_notification_thumbnails: false,
        reduced_motion: false,
        empty_text: "No notifications".to_string(),
        no_matching_text: "No matching notifications".to_string(),
        empty_offset_top: 24,
        empty_alignment: unixnotis_core::EmptyStateAlignment::Auto,
    }
}

pub(super) fn make_list() -> NotificationList {
    init_gtk();
    let scroller = gtk::ScrolledWindow::new();
    let (command_tx, _command_rx) = mpsc::channel::<UiCommand>(8);
    let (event_tx, _event_rx) = async_channel::bounded::<UiEvent>(8);
    NotificationList::new(
        scroller,
        command_tx,
        event_tx,
        Rc::new(IconResolver::new()),
        list_config(),
    )
}

pub(super) fn channels() -> (mpsc::Sender<UiCommand>, Sender<UiEvent>) {
    let (command_tx, _command_rx) = mpsc::channel::<UiCommand>(8);
    let (event_tx, _event_rx) = async_channel::bounded::<UiEvent>(8);
    (command_tx, event_tx)
}

pub(super) fn notification(id: u32, app_name: &str) -> NotificationView {
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
        is_transient: false,
        received_at_unix_seconds: 0,
        image: NotificationImage::default(),
    }
}
