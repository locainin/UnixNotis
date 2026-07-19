use gtk::gdk;
use gtk::prelude::*;

use super::{editable_has_focus, keyboard_action_for, KeyboardPanelAction};

#[test]
fn escape_closes_search_before_panel() {
    let state = gdk::ModifierType::empty();

    assert_eq!(
        keyboard_action_for(gdk::Key::Escape, state, true, false),
        KeyboardPanelAction::CloseSearch
    );
    assert_eq!(
        keyboard_action_for(gdk::Key::Escape, state, false, false),
        KeyboardPanelAction::ClosePanel
    );
}

#[test]
fn slash_and_ctrl_f_focus_search() {
    assert_eq!(
        keyboard_action_for(gdk::Key::slash, gdk::ModifierType::empty(), false, false),
        KeyboardPanelAction::FocusSearch
    );
    assert_eq!(
        keyboard_action_for(gdk::Key::f, gdk::ModifierType::CONTROL_MASK, false, false),
        KeyboardPanelAction::FocusSearch
    );
}

#[test]
fn ctrl_l_clears_and_focuses_search() {
    assert_eq!(
        keyboard_action_for(gdk::Key::l, gdk::ModifierType::CONTROL_MASK, true, false),
        KeyboardPanelAction::ClearAndFocusSearch
    );
}

#[test]
fn ctrl_w_toggles_widget_section() {
    assert_eq!(
        keyboard_action_for(gdk::Key::w, gdk::ModifierType::CONTROL_MASK, false, false),
        KeyboardPanelAction::ToggleWidgets
    );
    assert_eq!(
        keyboard_action_for(gdk::Key::w, gdk::ModifierType::empty(), false, false),
        KeyboardPanelAction::Continue
    );
}

#[test]
fn vim_scroll_keys_do_not_steal_editable_input() {
    let state = gdk::ModifierType::empty();

    assert_eq!(
        keyboard_action_for(gdk::Key::j, state, false, false),
        KeyboardPanelAction::ScrollDown
    );
    assert_eq!(
        keyboard_action_for(gdk::Key::k, state, false, false),
        KeyboardPanelAction::ScrollUp
    );
    assert_eq!(
        keyboard_action_for(gdk::Key::j, state, false, true),
        KeyboardPanelAction::Continue
    );
    assert_eq!(
        keyboard_action_for(gdk::Key::k, state, false, true),
        KeyboardPanelAction::Continue
    );
}

#[test]
fn all_panel_shortcuts_continue_while_an_editable_has_focus() {
    for (key, state) in [
        (gdk::Key::Escape, gdk::ModifierType::empty()),
        (gdk::Key::slash, gdk::ModifierType::empty()),
        (gdk::Key::j, gdk::ModifierType::empty()),
        (gdk::Key::k, gdk::ModifierType::empty()),
        (gdk::Key::f, gdk::ModifierType::CONTROL_MASK),
        (gdk::Key::l, gdk::ModifierType::CONTROL_MASK),
        (gdk::Key::w, gdk::ModifierType::CONTROL_MASK),
    ] {
        assert_eq!(
            keyboard_action_for(key, state, true, true),
            KeyboardPanelAction::Continue
        );
    }
}

#[test]
fn unrelated_keys_continue_to_gtk() {
    assert_eq!(
        keyboard_action_for(gdk::Key::space, gdk::ModifierType::empty(), false, false),
        KeyboardPanelAction::Continue
    );
}

#[gtk::test]
fn noneditable_focus_does_not_suppress_panel_shortcuts() {
    let window = gtk::Window::new();
    let button = gtk::Button::with_label("Focus target");
    window.set_child(Some(&button));
    window.set_visible(true);
    button.grab_focus();

    assert!(!editable_has_focus(&window));
}
