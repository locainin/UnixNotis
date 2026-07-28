use super::{
    build_action_row, build_body_label, build_close_button, build_header_spacer, build_reply_note,
    build_title_label, build_urgency_badge,
};
use gtk::prelude::*;
use unixnotis_core::{
    Action, AttributionClass, InlineReply, InlineReplyPolicy, NotificationAttribution,
    NotificationImage, NotificationView,
};

use crate::dbus::UiCommand;
use crate::ui::entry::presentation::{PopupEntryViewModel, ReplyPresentation};

#[gtk::test]
fn popup_critical_badge_uses_shared_hook_and_visibility() {
    let critical = build_urgency_badge(true);
    let normal = build_urgency_badge(false);

    assert!(critical.has_css_class(unixnotis_core::hooks::urgency::BADGE));
    assert_eq!(critical.text().as_str(), "Critical");
    assert!(critical.get_visible());
    assert!(!normal.get_visible());
}

#[gtk::test]
fn title_and_body_builders_keep_text_classes_and_line_limits() {
    let mut view = view_model();

    let title = build_title_label(&view).expect("visible title");
    let body = build_body_label(&view, 3).expect("visible body");

    assert_eq!(title.text().as_str(), "Primary title");
    assert!(title.has_css_class("unixnotis-popup-summary"));
    assert_eq!(title.lines(), 2);
    assert_eq!(body.text().as_str(), "Supporting body");
    assert!(body.has_css_class("unixnotis-popup-body"));
    assert_eq!(body.lines(), 3);

    view.title.clear();
    view.body = None;
    assert!(build_title_label(&view).is_none());
    assert!(build_body_label(&view, 3).is_none());
}

#[gtk::test]
fn reply_note_exists_only_when_the_policy_explanation_is_needed() {
    let mut view = view_model();
    assert!(build_reply_note(&view).is_none());

    view.trust.reply = ReplyPresentation::Unavailable;
    let note = build_reply_note(&view).expect("reply unavailable note");

    assert_eq!(note.text().as_str(), "Reply unavailable");
    assert!(note.has_css_class("unixnotis-popup-footer-note"));
}

#[gtk::test]
fn close_button_and_header_spacer_keep_their_interaction_contracts() {
    let close = build_close_button();
    let spacer = build_header_spacer();

    assert!(close.has_css_class("unixnotis-popup-close"));
    assert_eq!(
        close.tooltip_text().as_deref(),
        Some("Dismiss notification")
    );
    assert!(spacer.hexpands());
}

#[gtk::test]
fn action_row_dispatches_the_prepared_action_identity() {
    let (command_tx, mut command_rx) = tokio::sync::mpsc::channel(1);
    let view = view_model_with_action();
    let row = build_action_row(&command_tx, 41, &view).expect("action row");
    let button = row
        .first_child()
        .and_downcast::<gtk::Button>()
        .expect("action button");

    button.emit_clicked();

    match command_rx.try_recv().expect("queued action command") {
        UiCommand::InvokeAction { id, action_key } => {
            assert_eq!(id, 41);
            assert_eq!(action_key, "default");
        }
        command => panic!("unexpected command: {command:?}"),
    }
}

#[gtk::test]
fn extra_safe_action_builds_a_compact_overflow_menu() {
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel(1);
    let mut notification = notification();
    notification.actions = vec![
        Action {
            key: "default".to_string(),
            label: "Open".to_string(),
        },
        Action {
            key: "folder".to_string(),
            label: "Open folder".to_string(),
        },
    ];
    let view = PopupEntryViewModel::for_notification_at(&notification, 1_000);
    let row = build_action_row(&command_tx, 41, &view).expect("action row");
    let menu = row
        .last_child()
        .and_downcast::<gtk::MenuButton>()
        .expect("overflow menu");

    assert_eq!(menu.icon_name().as_deref(), Some("view-more-symbolic"));
    assert!(menu.popover().is_some());
}

#[gtk::test]
fn empty_action_model_does_not_build_an_action_row() {
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel(1);
    assert!(build_action_row(&command_tx, 41, &view_model()).is_none());
}

fn view_model() -> PopupEntryViewModel {
    PopupEntryViewModel::for_notification_at(&notification(), 1_000)
}

fn view_model_with_action() -> PopupEntryViewModel {
    let mut notification = notification();
    notification.actions.push(Action {
        key: "default".to_string(),
        label: "Open".to_string(),
    });
    PopupEntryViewModel::for_notification_at(&notification, 1_000)
}

fn notification() -> NotificationView {
    NotificationView {
        id: 41,
        generation: 3,
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
        summary: "Primary title".to_string(),
        body: "Supporting body".to_string(),
        actions: Vec::new(),
        inline_reply: InlineReply::default(),
        inline_reply_policy: InlineReplyPolicy::Allow,
        urgency: 1,
        category: String::new(),
        is_transient: false,
        received_at_unix_seconds: 1_000,
        image: NotificationImage::default(),
    }
}
