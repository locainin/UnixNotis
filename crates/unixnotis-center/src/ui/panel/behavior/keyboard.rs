//! Keyboard shortcut wiring for the panel

use gtk::gdk;
use gtk::prelude::*;

use crate::control::UiCommand;
use crate::ui::try_send_command;

use super::super::widgets::PanelWidgets;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum KeyboardPanelAction {
    // Search is open, so Escape closes search before closing the whole panel
    CloseSearch,
    // Panel close is the final Escape behavior
    ClosePanel,
    // Search should be revealed and focused
    FocusSearch,
    // Search should be revealed, cleared, and focused
    ClearAndFocusSearch,
    // Widget group collapse toggle
    ToggleWidgets,
    // Vertical scroll nudge for keyboard navigation
    ScrollDown,
    // Vertical scroll nudge for keyboard navigation
    ScrollUp,
    // GTK can continue handling the key
    Continue,
}

pub(super) fn keyboard_action_for(
    key: gdk::Key,
    state: gdk::ModifierType,
    search_open: bool,
    editable_has_focus: bool,
) -> KeyboardPanelAction {
    if editable_has_focus {
        // Editable widgets own typing and Escape while a draft or query has focus
        return KeyboardPanelAction::Continue;
    }

    if key == gdk::Key::Escape {
        return if search_open {
            KeyboardPanelAction::CloseSearch
        } else {
            KeyboardPanelAction::ClosePanel
        };
    }

    if key == gdk::Key::slash
        || (key == gdk::Key::f && state.contains(gdk::ModifierType::CONTROL_MASK))
    {
        return KeyboardPanelAction::FocusSearch;
    }

    if key == gdk::Key::l && state.contains(gdk::ModifierType::CONTROL_MASK) {
        return KeyboardPanelAction::ClearAndFocusSearch;
    }

    if key == gdk::Key::w && state.contains(gdk::ModifierType::CONTROL_MASK) {
        return KeyboardPanelAction::ToggleWidgets;
    }

    if key == gdk::Key::j {
        return KeyboardPanelAction::ScrollDown;
    }

    if key == gdk::Key::k {
        return KeyboardPanelAction::ScrollUp;
    }

    KeyboardPanelAction::Continue
}

pub(in crate::ui) fn connect_keyboard_shortcuts(
    panel: &PanelWidgets,
    command_tx: tokio::sync::mpsc::Sender<UiCommand>,
) {
    let focus_toggle = panel.header.actions.focus_toggle.clone();
    let search_toggle = panel.header.actions.search_toggle.clone();
    let search_revealer = panel.header.search.revealer.clone();
    let search_entry = panel.header.search.entry.clone();
    let scroller = panel.sections.scroller.clone();
    let window = panel.window.clone();
    let key_controller = gtk::EventControllerKey::new();

    key_controller.connect_key_pressed(move |_, key, _, state| {
        let action = keyboard_action_for(
            key,
            state,
            search_revealer.reveals_child(),
            editable_has_focus(&window),
        );
        match action {
            KeyboardPanelAction::CloseSearch => {
                search_toggle.set_active(false);
                gtk::glib::Propagation::Stop
            }
            KeyboardPanelAction::ClosePanel => {
                try_send_command(&command_tx, UiCommand::ClosePanel);
                gtk::glib::Propagation::Stop
            }
            KeyboardPanelAction::FocusSearch => {
                reveal_and_focus_search(&search_toggle, &search_revealer, &search_entry);
                gtk::glib::Propagation::Stop
            }
            KeyboardPanelAction::ClearAndFocusSearch => {
                reveal_and_focus_search(&search_toggle, &search_revealer, &search_entry);
                search_entry.set_text("");
                gtk::glib::Propagation::Stop
            }
            KeyboardPanelAction::ToggleWidgets => {
                focus_toggle.set_active(!focus_toggle.is_active());
                gtk::glib::Propagation::Stop
            }
            KeyboardPanelAction::ScrollDown => {
                nudge_scroller(&scroller, 72.0);
                gtk::glib::Propagation::Stop
            }
            KeyboardPanelAction::ScrollUp => {
                nudge_scroller(&scroller, -72.0);
                gtk::glib::Propagation::Stop
            }
            KeyboardPanelAction::Continue => gtk::glib::Propagation::Proceed,
        }
    });
    panel.root.add_controller(key_controller);
}

#[cfg(test)]
#[path = "tests/keyboard.rs"]
mod tests;

pub(in crate::ui) fn editable_has_focus(window: &impl IsA<gtk::Window>) -> bool {
    gtk::prelude::RootExt::focus(window.as_ref()).is_some_and(|widget| widget.is::<gtk::Editable>())
}

fn reveal_and_focus_search(
    search_toggle: &gtk::ToggleButton,
    search_revealer: &gtk::Revealer,
    search_entry: &gtk::SearchEntry,
) {
    if !search_revealer.reveals_child() {
        // Toggle owns the reveal transition and guard logic
        search_toggle.set_active(true);
    }
    search_entry.grab_focus();
    search_entry.select_region(0, -1);
}

fn nudge_scroller(scroller: &gtk::ScrolledWindow, delta: f64) {
    let adjustment = scroller.vadjustment();
    let upper = (adjustment.upper() - adjustment.page_size()).max(adjustment.lower());
    let next = (adjustment.value() + delta).clamp(adjustment.lower(), upper);
    adjustment.set_value(next);
}
