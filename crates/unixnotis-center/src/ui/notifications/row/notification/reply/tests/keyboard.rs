//! Inline reply keyboard and editable-focus tests

use std::cell::Cell;

use gtk::prelude::*;

use crate::ui::notifications::test_support::init_gtk;
use crate::ui::panel::behavior::keyboard::editable_has_focus;

use super::{build_inline_reply, build_notification_row, cancel_inline_reply};

#[gtk::test]
fn inline_reply_escape_clears_an_idle_draft_and_collapses_the_form() {
    init_gtk();
    let entry = gtk::Entry::new();
    let revealer = gtk::Revealer::new();
    let error_label = gtk::Label::new(Some("Could not send"));
    let submitted = Cell::new(false);
    entry.set_text("Unsent draft");
    revealer.set_reveal_child(true);
    error_label.set_visible(true);

    assert_eq!(
        cancel_inline_reply(&entry, &revealer, &error_label, &submitted),
        gtk::glib::Propagation::Stop
    );
    assert!(entry.text().is_empty());
    assert!(!revealer.reveals_child());
    assert!(error_label.text().is_empty());
    assert!(!error_label.is_visible());
}

#[gtk::test]
fn inline_reply_key_controller_cancels_only_escape() {
    init_gtk();
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel(4);
    let widgets = build_inline_reply(command_tx);
    widgets.entry.set_text("Unsent draft");
    widgets.revealer.set_reveal_child(true);
    let controllers = widgets.entry.observe_controllers();
    let controller = (0..controllers.n_items())
        .filter_map(|index| controllers.item(index))
        .find_map(|object| object.downcast::<gtk::EventControllerKey>().ok())
        .expect("inline reply key controller");

    let proceed = controller.emit_by_name::<bool>(
        "key-pressed",
        &[&gtk::gdk::Key::a, &0_u32, &gtk::gdk::ModifierType::empty()],
    );
    assert!(!proceed);
    assert_eq!(widgets.entry.text(), "Unsent draft");

    let stop = controller.emit_by_name::<bool>(
        "key-pressed",
        &[
            &gtk::gdk::Key::Escape,
            &0_u32,
            &gtk::gdk::ModifierType::empty(),
        ],
    );
    assert!(stop);
    assert!(widgets.entry.text().is_empty());
    assert!(!widgets.revealer.reveals_child());
}

#[gtk::test]
fn inline_reply_entry_focus_is_recognized_as_editable_panel_input() {
    init_gtk();
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel(4);
    let (root, row) = build_notification_row(command_tx);
    let window = gtk::Window::new();
    window.set_child(Some(&root));
    window.set_visible(true);

    row.inline_reply.entry.grab_focus();

    assert!(editable_has_focus(&window));
}
