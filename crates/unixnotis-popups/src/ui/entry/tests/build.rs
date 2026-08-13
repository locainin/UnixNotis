use super::{connect_close_action, connect_hover_events};
use crate::dbus::UiEvent;
use crate::ui::entry::activation::connect_default_action;
use gtk::prelude::*;
use unixnotis_core::{
    Action, AttributionReason, InlineReply, InlineReplyPolicy, NotificationAttribution,
    NotificationImage, NotificationView,
};

use crate::dbus::UiCommand;
use crate::ui::entry::presentation::PopupEntryViewModel;

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

    assert_eq!(root.observe_controllers().n_items(), 2);
    assert!(root.is_focusable());
}

#[gtk::test]
fn blank_default_action_still_adds_card_click_handling() {
    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel(1);
    let mut view = notification();
    view.actions.push(Action {
        key: "default".to_string(),
        label: String::new(),
    });
    let model = PopupEntryViewModel::for_notification_at(&view, 1_000);

    connect_default_action(&root, view.key(), &model, &command_tx);

    assert_eq!(root.observe_controllers().n_items(), 2);
    assert!(root.is_focusable());
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

#[gtk::test]
fn card_level_motion_controller_emits_generation_keyed_hover_state() {
    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let notification = notification();
    let (event_tx, event_rx) = async_channel::bounded(2);
    connect_hover_events(&root, notification.key(), &event_tx);

    let controllers = root.observe_controllers();
    assert_eq!(controllers.n_items(), 1);
    let motion = controllers
        .item(0)
        .and_downcast::<gtk::EventControllerMotion>()
        .expect("card controller should handle pointer motion");
    motion.emit_by_name::<()>("enter", &[&0.0_f64, &0.0_f64]);
    while gtk::glib::MainContext::default().pending() {
        gtk::glib::MainContext::default().iteration(false);
    }

    assert!(matches!(
        event_rx.try_recv().expect("pointer enter event"),
        UiEvent::PopupHoverChanged(key, true) if key == notification.key()
    ));

    motion.emit_by_name::<()>("leave", &[]);
    while gtk::glib::MainContext::default().pending() {
        gtk::glib::MainContext::default().iteration(false);
    }
    assert!(matches!(
        event_rx.try_recv().expect("pointer leave event"),
        UiEvent::PopupHoverChanged(key, false) if key == notification.key()
    ));
}

#[gtk::test]
fn saturated_hover_queue_coalesces_to_one_final_pointer_state() {
    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let notification = notification();
    let (event_tx, event_rx) = async_channel::bounded(1);
    event_tx
        .try_send(UiEvent::CssReload)
        .expect("prefill hover event queue");
    connect_hover_events(&root, notification.key(), &event_tx);
    let motion = root
        .observe_controllers()
        .item(0)
        .and_downcast::<gtk::EventControllerMotion>()
        .expect("card controller should handle pointer motion");

    for _ in 0..64 {
        motion.emit_by_name::<()>("enter", &[&0.0_f64, &0.0_f64]);
        motion.emit_by_name::<()>("leave", &[]);
    }
    while gtk::glib::MainContext::default().pending() {
        gtk::glib::MainContext::default().iteration(false);
    }
    assert!(matches!(
        event_rx.try_recv().expect("prefilled event"),
        UiEvent::CssReload
    ));
    while gtk::glib::MainContext::default().pending() {
        gtk::glib::MainContext::default().iteration(false);
    }

    assert!(matches!(
        event_rx.try_recv().expect("coalesced final hover state"),
        UiEvent::PopupHoverChanged(key, false) if key == notification.key()
    ));
    while gtk::glib::MainContext::default().pending() {
        gtk::glib::MainContext::default().iteration(false);
    }
    assert!(
        event_rx.try_recv().is_err(),
        "coalescing must not leave more pointer sends waiting"
    );
}

pub(super) fn notification() -> NotificationView {
    NotificationView {
        id: 31,
        generation: 1,
        app_name: "Example".to_string(),
        attribution: NotificationAttribution::verified(
            "Example",
            "Example",
            "org.example.App",
            "example-app",
            AttributionReason::ExactSystemExecutable,
            "exact system executable",
            "system-app:org.example.App".to_string(),
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
        popup_hide_after_ms: 0,
    }
}
