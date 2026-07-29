use gtk::prelude::*;
use unixnotis_core::NotificationKey;

use super::{
    connect_default_action, dispatch_default_action, handle_default_action_key,
    is_default_activation_key, mark_interactive,
};
use crate::dbus::UiCommand;
use crate::ui::entry::presentation::{PopupEntryViewModel, PopupKind, ReplyPresentation};
use unixnotis_ui::presentation::{BadgePresentation, ThumbnailKind, TrustLevel, TrustPresentation};

const KEY: NotificationKey = NotificationKey {
    id: 41,
    generation: 3,
};

#[gtk::test]
fn clicking_overflow_menu_does_not_invoke_default() {
    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let menu = gtk::MenuButton::new();
    mark_interactive(&menu);
    root.append(&menu);
    assert_pick_does_not_dispatch(&root, &menu);
}

#[gtk::test]
fn clicking_reply_entry_does_not_invoke_default() {
    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let reply = gtk::Box::new(gtk::Orientation::Vertical, 0);
    mark_interactive(&reply);
    let entry = gtk::Entry::new();
    reply.append(&entry);
    root.append(&reply);
    assert_pick_does_not_dispatch(&root, &entry);
}

#[gtk::test]
fn clicking_reply_button_does_not_invoke_default() {
    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let button = gtk::Button::with_label("Reply");
    mark_interactive(&button);
    root.append(&button);
    assert_pick_does_not_dispatch(&root, &button);
}

#[gtk::test]
fn clicking_plain_card_content_invokes_default_once() {
    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let label = gtk::Label::new(Some("Message"));
    root.append(&label);
    let (command_tx, mut command_rx) = tokio::sync::mpsc::channel(2);

    dispatch_default_action(
        root.upcast_ref(),
        Some(label.upcast()),
        KEY,
        "default",
        &command_tx,
    );

    assert_default_command(&mut command_rx);
    assert!(command_rx.try_recv().is_err());
}

#[gtk::test]
fn default_action_card_is_focusable_and_keyboard_activatable() {
    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel(2);
    let view = PopupEntryViewModel {
        kind: PopupKind::Utility,
        app_label: "Example".to_string(),
        secondary_claim: None,
        badge: BadgePresentation::AuthenticatedApplication,
        timestamp_label: "now".to_string(),
        title: "Update complete".to_string(),
        body: None,
        thumbnail: ThumbnailKind::None,
        default_action_key: Some("default".to_string()),
        primary_actions: Vec::new(),
        overflow_actions: Vec::new(),
        trust: TrustPresentation {
            level: TrustLevel::Verified,
            short_label: None,
            details_label: None,
            reply: ReplyPresentation::Hidden,
        },
        critical: false,
    };

    connect_default_action(&root, KEY, &view, &command_tx);

    assert!(root.is_focusable());
    assert_eq!(root.accessible_role(), gtk::AccessibleRole::Button);
    assert!(is_default_activation_key(gtk::gdk::Key::Return));
    assert!(is_default_activation_key(gtk::gdk::Key::KP_Enter));
    assert!(is_default_activation_key(gtk::gdk::Key::space));
    assert!(!is_default_activation_key(gtk::gdk::Key::Escape));
}

#[gtk::test]
fn keyboard_default_action_requires_card_focus_and_enter_or_space() {
    for key in [
        gtk::gdk::Key::Return,
        gtk::gdk::Key::KP_Enter,
        gtk::gdk::Key::space,
    ] {
        let (command_tx, mut command_rx) = tokio::sync::mpsc::channel(1);
        assert_eq!(
            handle_default_action_key(true, key, KEY, "default", &command_tx),
            gtk::glib::Propagation::Stop
        );
        assert_default_command(&mut command_rx);
    }

    for (focused, key) in [
        (false, gtk::gdk::Key::Return),
        (true, gtk::gdk::Key::Escape),
        (false, gtk::gdk::Key::Escape),
    ] {
        let (command_tx, mut command_rx) = tokio::sync::mpsc::channel(1);
        assert_eq!(
            handle_default_action_key(focused, key, KEY, "default", &command_tx),
            gtk::glib::Propagation::Proceed
        );
        assert!(command_rx.try_recv().is_err());
    }
}

fn assert_pick_does_not_dispatch<W: IsA<gtk::Widget>>(root: &gtk::Box, picked: &W) {
    let (command_tx, mut command_rx) = tokio::sync::mpsc::channel(1);
    dispatch_default_action(
        root.upcast_ref(),
        Some(picked.clone().upcast()),
        KEY,
        "default",
        &command_tx,
    );
    assert!(command_rx.try_recv().is_err());
}

fn assert_default_command(command_rx: &mut tokio::sync::mpsc::Receiver<UiCommand>) {
    match command_rx.try_recv().expect("default action command") {
        UiCommand::InvokeAction {
            notification,
            action_key,
        } => {
            assert_eq!(notification, KEY);
            assert_eq!(action_key, "default");
        }
        command => panic!("unexpected command: {command:?}"),
    }
}
