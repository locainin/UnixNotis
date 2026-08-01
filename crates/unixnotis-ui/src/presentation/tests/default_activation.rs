use super::super::default_activation::{
    connect_default_activation, is_default_activation_key, keyboard_activation_is_ready,
    mark_interactive, picked_widget_blocks_default_action, DefaultActionTarget,
};
use gtk::prelude::*;
use unixnotis_core::NotificationKey;

#[test]
fn activation_keys_match_pointer_equivalents() {
    assert!(is_default_activation_key(gtk::gdk::Key::Return));
    assert!(is_default_activation_key(gtk::gdk::Key::KP_Enter));
    assert!(is_default_activation_key(gtk::gdk::Key::space));
    assert!(!is_default_activation_key(gtk::gdk::Key::Escape));
}

#[gtk::test]
fn binding_replaces_and_clears_the_current_generation() {
    let card = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let binding = connect_default_activation(&card, |_, _| {});
    let first = DefaultActionTarget {
        notification: NotificationKey {
            id: 7,
            generation: 1,
        },
        action_key: "default".to_string(),
    };
    binding.set_target(Some(first.clone()));
    assert_eq!(binding.target.borrow().as_ref(), Some(&first));
    let replacement = DefaultActionTarget {
        notification: NotificationKey {
            id: 8,
            generation: 2,
        },
        action_key: "open".to_string(),
    };
    binding.set_target(Some(replacement.clone()));
    assert_eq!(binding.target.borrow().as_ref(), Some(&replacement));
    binding.set_target(None);
    assert!(binding.target.borrow().is_none());
}

#[gtk::test]
fn interactive_descendants_block_card_activation() {
    let card = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let marked = gtk::Box::new(gtk::Orientation::Vertical, 0);
    marked.set_focusable(false);
    mark_interactive(&marked);
    card.append(&marked);

    assert!(picked_widget_blocks_default_action(
        card.upcast_ref(),
        Some(marked.upcast())
    ));
}

#[gtk::test]
fn focusable_or_marked_descendants_block_but_plain_content_does_not() {
    let card = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let focusable = gtk::Box::new(gtk::Orientation::Vertical, 0);
    focusable.set_focusable(true);
    let plain = gtk::Label::new(Some("Message"));
    let menu = gtk::MenuButton::new();
    menu.set_focusable(false);
    let entry = gtk::Entry::new();
    entry.set_focusable(false);
    card.append(&focusable);
    card.append(&plain);
    card.append(&menu);
    card.append(&entry);

    assert!(picked_widget_blocks_default_action(
        card.upcast_ref(),
        Some(focusable.upcast())
    ));
    assert!(!picked_widget_blocks_default_action(
        card.upcast_ref(),
        Some(plain.upcast())
    ));
    assert!(picked_widget_blocks_default_action(
        card.upcast_ref(),
        Some(menu.upcast())
    ));
    assert!(picked_widget_blocks_default_action(
        card.upcast_ref(),
        Some(entry.upcast())
    ));
}

#[test]
fn keyboard_activation_requires_focus_key_and_target() {
    assert!(keyboard_activation_is_ready(
        true,
        gtk::gdk::Key::Return,
        true
    ));
    assert!(!keyboard_activation_is_ready(
        false,
        gtk::gdk::Key::Return,
        true
    ));
    assert!(!keyboard_activation_is_ready(
        true,
        gtk::gdk::Key::Escape,
        true
    ));
    assert!(!keyboard_activation_is_ready(
        true,
        gtk::gdk::Key::Return,
        false
    ));
}
