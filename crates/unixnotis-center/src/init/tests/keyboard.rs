use gtk::gdk;

use super::keyboard::{keyboard_action_for, KeyboardPanelAction};

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
}

#[test]
fn vim_scroll_keys_do_not_steal_text_entry_input() {
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
fn unrelated_keys_continue_to_gtk() {
    assert_eq!(
        keyboard_action_for(gdk::Key::space, gdk::ModifierType::empty(), false, false),
        KeyboardPanelAction::Continue
    );
}
