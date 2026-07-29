use super::{connect_close_action, connect_default_action, widget_type_blocks_default_action};
use gtk::glib::prelude::StaticType;
use gtk::prelude::*;
use unixnotis_core::{
    Action, AttributionClass, InlineReply, InlineReplyPolicy, NotificationAttribution,
    NotificationImage, NotificationView,
};

use crate::dbus::UiCommand;
use crate::ui::entry::presentation::PopupEntryViewModel;

#[gtk::test]
fn default_card_action_is_blocked_for_button_widgets() {
    // Button clicks must remain owned by the button action
    assert!(widget_type_blocks_default_action(gtk::Button::static_type()));
}

#[gtk::test]
fn default_card_action_is_allowed_for_plain_content_widgets() {
    // Plain card content may use the notification default action
    assert!(!widget_type_blocks_default_action(gtk::Label::static_type()));
}

#[gtk::test]
fn close_button_dispatches_only_the_notification_dismissal() {
    let close = gtk::Button::new();
    let (command_tx, mut command_rx) = tokio::sync::mpsc::channel(1);
    let notification = notification();
    connect_close_action(&close, notification.key(), &command_tx);

    close.emit_clicked();

    match command_rx.try_recv().expect("queued dismiss command") {
        UiCommand::Dismiss(key) => assert_eq!(key, notification.key()),
        command => panic!("unexpected command: {command:?}"),
    }
}

#[gtk::test]
fn exact_default_action_adds_card_click_handling() {
    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel(1);
    let mut view = notification();
    view.actions.push(Action {
        key: "default".to_string(),
        label: "Open".to_string(),
    });
    let model = PopupEntryViewModel::for_notification_at(&view, 1_000);

    connect_default_action(&root, view.key(), &model, &command_tx);

    assert_eq!(root.observe_controllers().n_items(), 1);
}

#[gtk::test]
fn nondefault_action_does_not_make_the_whole_card_clickable() {
    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel(1);
    let mut view = notification();
    view.actions.push(Action {
        key: "details".to_string(),
        label: "Details".to_string(),
    });
    let model = PopupEntryViewModel::for_notification_at(&view, 1_000);

    connect_default_action(&root, view.key(), &model, &command_tx);

    assert_eq!(root.observe_controllers().n_items(), 0);
}

fn notification() -> NotificationView {
    NotificationView {
        id: 31,
        generation: 1,
        app_name: "Example".to_string(),
        attribution: NotificationAttribution::associated(
            "Example",
            "org.example.App",
            "org.example.App",
            "",
            AttributionClass::SystemAssociated,
            false,
            "system-desktop:org.example.App".to_string(),
        ),
        summary: "Example".to_string(),
        body: String::new(),
        actions: Vec::new(),
        inline_reply: InlineReply::default(),
        inline_reply_policy: InlineReplyPolicy::Allow,
        urgency: 1,
        category: String::new(),
        is_transient: false,
        received_at_unix_seconds: 1_000,
        image: NotificationImage::default(),
        popup_decision: unixnotis_core::PopupDecisionRecord::default(),
    }
}
