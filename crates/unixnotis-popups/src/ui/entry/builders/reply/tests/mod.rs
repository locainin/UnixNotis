use gtk::prelude::*;
use unixnotis_core::{
    Action, AttributionReason, InlineReply, InlineReplyPolicy, NotificationAttribution,
    NotificationImage, NotificationView,
};

use super::super::build_inline_reply;
use crate::dbus::UiCommand;
use crate::ui::entry::presentation::PopupEntryViewModel;

#[gtk::test]
fn reply_button_reveals_editor_without_sending_and_submission_keeps_generation() {
    let mut notification = notification();
    notification.inline_reply = InlineReply {
        available: true,
        label: "Reply".to_string(),
        ..InlineReply::default()
    };
    notification.actions.push(Action {
        key: "inline-reply".to_string(),
        label: "Reply".to_string(),
    });
    let view = PopupEntryViewModel::for_notification_at(&notification, 1_000);
    let (command_tx, mut command_rx) = tokio::sync::mpsc::channel(1);
    let widget =
        build_inline_reply(&notification, &view, &command_tx).expect("verified reply editor");
    let reveal = widget
        .first_child()
        .and_downcast::<gtk::Button>()
        .expect("reply button");
    let revealer = widget
        .last_child()
        .and_downcast::<gtk::Revealer>()
        .expect("reply revealer");

    reveal.emit_clicked();
    assert!(revealer.reveals_child());
    assert!(command_rx.try_recv().is_err());

    let form = revealer
        .child()
        .and_downcast::<gtk::Box>()
        .expect("reply form");
    let input_row = form
        .first_child()
        .and_downcast::<gtk::Box>()
        .expect("reply input row");
    let entry = input_row
        .first_child()
        .and_downcast::<gtk::Entry>()
        .expect("reply entry");
    entry.set_text("On my way");
    entry.emit_activate();

    let UiCommand::Reply {
        id,
        generation,
        text,
        ..
    } = command_rx.try_recv().expect("reply command")
    else {
        panic!("expected reply command");
    };
    assert_eq!(id, notification.id);
    assert_eq!(generation, notification.generation);
    assert_eq!(text, "On my way");
}

#[gtk::test]
fn unverified_notification_never_builds_a_reply_editor() {
    let mut notification = notification();
    notification.inline_reply.available = true;
    notification.inline_reply_policy = unixnotis_core::InlineReplyPolicy::Deny;
    let view = PopupEntryViewModel::for_notification_at(&notification, 1_000);
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel(1);

    assert!(build_inline_reply(&notification, &view, &command_tx).is_none());
}

fn notification() -> NotificationView {
    NotificationView {
        id: 7,
        generation: 11,
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
        summary: "New message".to_string(),
        body: "Are you coming?".to_string(),
        actions: Vec::new(),
        inline_reply: InlineReply::default(),
        inline_reply_policy: InlineReplyPolicy::Allow,
        urgency: 1,
        category: "im.received".to_string(),
        is_transient: false,
        received_at_unix_seconds: 1_000,
        image: NotificationImage::default(),
        popup_decision: unixnotis_core::PopupDecisionRecord::default(),
    }
}
