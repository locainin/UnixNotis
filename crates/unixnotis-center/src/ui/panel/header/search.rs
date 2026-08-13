//! Panel search construction, filtering, and reveal wiring

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use async_channel::TrySendError;
use gtk::prelude::*;
use unixnotis_core::{css::hooks, PanelConfig};

use super::super::body::WIDGET_REVEAL_TRANSITION_MS;
use crate::control::UiEvent;
use crate::ui::panel::behavior::input::{ClickCooldown, LatestBoolEventGate};

pub const SEARCH_REVEAL_TRANSITION_MS: u64 = 180;
const WIDGETS_TOGGLE_COALESCE_MS: u64 = 16;

pub(in crate::ui) struct PanelSearchWidgets {
    pub(in crate::ui) revealer: gtk::Revealer,
    pub(in crate::ui) entry: gtk::SearchEntry,
    pub(in crate::ui) magnifier: gtk::Image,
    pub(in crate::ui) clear_button: gtk::Button,
}

pub(super) fn build_panel_search(config: &PanelConfig) -> PanelSearchWidgets {
    let search_shell = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    search_shell.add_css_class(hooks::panel_shell::SEARCH_SHELL);
    search_shell.set_hexpand(true);

    let leading_accent = gtk::Box::new(gtk::Orientation::Vertical, 0);
    leading_accent.add_css_class(hooks::panel_shell::SEARCH_ACCENT);
    leading_accent.add_css_class(hooks::panel_shell::TICK_TOP_LEFT);

    let star_accent = gtk::Label::new(Some("*"));
    star_accent.add_css_class(hooks::panel_shell::SEARCH_STAR);

    let magnifier = gtk::Image::from_icon_name(&config.search_magnifier_icon);
    magnifier.add_css_class(hooks::panel_shell::SEARCH_MAGNIFIER);
    magnifier.set_accessible_role(gtk::AccessibleRole::Presentation);

    let search_entry = gtk::SearchEntry::new();
    search_entry.add_css_class(hooks::panel_shell::SEARCH);
    // Native icons have no public child or dedicated CSS node, so owned siblings replace them
    search_entry.add_css_class(hooks::panel_shell::SEARCH_OWNED_ICONS);
    // Placeholder text keeps the intent obvious before the first query
    search_entry.set_placeholder_text(Some(&config.search_placeholder));
    search_entry.set_hexpand(true);
    search_entry.set_tooltip_text(Some("Type to filter notifications"));

    let clear_button = gtk::Button::from_icon_name("edit-clear-symbolic");
    clear_button.add_css_class(hooks::panel_shell::SEARCH_CLEAR);
    clear_button.set_tooltip_text(Some("Clear search"));
    clear_button.set_visible(false);
    let clear_entry = search_entry.clone();
    clear_button.connect_clicked(move |_| clear_entry.set_text(""));
    let visible_clear = clear_button.clone();
    search_entry.connect_changed(move |entry| {
        // The clear action exists only while a query can be removed
        visible_clear.set_visible(!entry.text().is_empty());
    });

    search_shell.append(&leading_accent);
    search_shell.append(&magnifier);
    search_shell.append(&search_entry);
    search_shell.append(&clear_button);
    search_shell.append(&star_accent);

    let search_revealer = gtk::Revealer::new();
    search_revealer.add_css_class(hooks::panel_shell::SEARCH_REVEALER);
    // Slide-down matches the rest of the panel reveal motion
    search_revealer.set_transition_type(gtk::RevealerTransitionType::SlideDown);
    search_revealer.set_transition_duration(SEARCH_REVEAL_TRANSITION_MS as u32);
    // Keep search hidden until the user asks for it so notifications keep the space
    search_revealer.set_reveal_child(config.search_visible);
    search_revealer.set_child(Some(&search_shell));

    PanelSearchWidgets {
        revealer: search_revealer,
        entry: search_entry,
        magnifier,
        clear_button,
    }
}

pub(in crate::ui) fn connect_widget_collapse_toggle(
    focus_toggle: &gtk::ToggleButton,
    widget_revealer: &gtk::Revealer,
    event_tx: async_channel::Sender<UiEvent>,
) {
    let collapse_gate = LatestBoolEventGate::new(Duration::from_millis(WIDGETS_TOGGLE_COALESCE_MS));
    let collapse_click_gate =
        ClickCooldown::new(Duration::from_millis(WIDGET_REVEAL_TRANSITION_MS));
    let accepted_collapsed = Rc::new(Cell::new(false));
    // Restore guard prevents a rejected click rollback from re-entering this handler
    let collapse_restore = Rc::new(Cell::new(false));
    let collapse_revealer = widget_revealer.clone();

    focus_toggle.connect_toggled(move |button| {
        if collapse_restore.replace(false) {
            return;
        }

        let collapsed = button.is_active();
        // Ignore clicks while the previous reveal animation is still changing layout
        if !try_start_reveal_transition(&collapse_click_gate, &collapse_revealer) {
            let accepted = accepted_collapsed.get();
            if collapsed != accepted {
                // Roll back only the rejected edge so the UI mirrors the running transition
                collapse_restore.set(true);
                button.set_active(accepted);
            }
            return;
        }

        accepted_collapsed.set(collapsed);
        hold_button_for_reveal_transition(button, &collapse_revealer);
        collapse_gate.request_widgets_collapsed(&event_tx, collapsed);
    });
}

pub(in crate::ui) fn connect_filter_entry(
    search_entry: &gtk::SearchEntry,
    event_tx: async_channel::Sender<UiEvent>,
) {
    // SearchChanged covers typing, clear actions, and programmatic text resets
    search_entry.connect_search_changed(move |entry| {
        send_filter_event(&event_tx, entry.text().to_string());
    });
}

pub(super) fn send_filter_event(event_tx: &async_channel::Sender<UiEvent>, filter: String) {
    let event = UiEvent::FilterChanged(filter);
    match event_tx.try_send(event) {
        Ok(()) => {}
        Err(TrySendError::Full(event)) => {
            // Search changes are small and should retry instead of disappearing under bursts
            let event_tx = event_tx.clone();
            gtk::glib::MainContext::default().spawn_local(async move {
                let _ = event_tx.send(event).await;
            });
        }
        Err(TrySendError::Closed(_)) => {} // A closed UI channel means shutdown already owns the pending filter state
    }
}

pub(in crate::ui) fn set_search_open(
    search_toggle: &gtk::ToggleButton,
    search_revealer: &gtk::Revealer,
    search_entry: &gtk::SearchEntry,
    search_toggle_guard: &Cell<bool>,
    open: bool,
) {
    // Guarded changes still pass through the signal handler when the toggle changes
    let previous_guard = search_toggle_guard.replace(true);
    search_toggle.set_active(open);
    // Nested callers retain the guard state owned by the outer operation
    search_toggle_guard.set(previous_guard);

    // Apply directly as well because GTK emits no signal when the toggle already matches
    apply_search_open_state(search_revealer, search_entry, open);
}

pub(in crate::ui) fn connect_search_toggle(
    search_toggle: &gtk::ToggleButton,
    search_revealer: &gtk::Revealer,
    search_entry: &gtk::SearchEntry,
    search_toggle_guard: Rc<Cell<bool>>,
) {
    let search_click_gate = ClickCooldown::new(Duration::from_millis(SEARCH_REVEAL_TRANSITION_MS));
    // Programmatic rollback must not be mistaken for a fresh user click
    let search_restore = Rc::new(Cell::new(false));
    let toggled_revealer = search_revealer.clone();
    let toggled_entry = search_entry.clone();

    // A weak reference avoids a signal cycle between the entry and toggle
    let stop_toggle = search_toggle.downgrade();
    let stop_revealer = search_revealer.clone();
    let stop_entry = search_entry.clone();
    let stop_click_gate = search_click_gate.clone();
    let stop_guard = search_toggle_guard.clone();
    search_entry.connect_stop_search(move |_| {
        // Escape is a semantic close and should not wait for the reveal click cooldown
        stop_click_gate.release();
        if let Some(toggle) = stop_toggle.upgrade() {
            set_search_open(
                &toggle,
                &stop_revealer,
                &stop_entry,
                stop_guard.as_ref(),
                false,
            );
        } else {
            // The entry may briefly outlive its toggle during GTK teardown
            apply_search_open_state(&stop_revealer, &stop_entry, false);
        }
    });

    search_toggle.connect_toggled(move |button| {
        if search_restore.replace(false) {
            return;
        }

        let reveal = button.is_active();
        if search_toggle_guard.get() {
            // Programmatic changes must keep every search widget on the same state
            search_click_gate.release();
            apply_search_open_state(&toggled_revealer, &toggled_entry, reveal);
            return;
        }

        if !try_start_reveal_transition(&search_click_gate, &toggled_revealer) {
            // The revealer records the last accepted transition target
            let accepted = toggled_revealer.reveals_child();
            if reveal != accepted {
                // Keep the visual toggle synced with the accepted revealer state
                search_restore.set(true);
                button.set_active(accepted);
            }
            return;
        }

        hold_button_for_reveal_transition(button, &toggled_revealer);
        apply_search_open_state(&toggled_revealer, &toggled_entry, reveal);
        if reveal {
            // Selecting existing text makes the next query replace it immediately
            toggled_entry.grab_focus();
            toggled_entry.select_region(0, i32::MAX);
        }
    });
}

fn try_start_reveal_transition(gate: &ClickCooldown, revealer: &gtk::Revealer) -> bool {
    if revealer.transition_duration() == 0 {
        // Immediate transitions have no in-flight layout window to guard
        gate.release();
        return true;
    }

    gate.try_start()
}

fn hold_button_for_reveal_transition(button: &gtk::ToggleButton, revealer: &gtk::Revealer) {
    let duration_ms = revealer.transition_duration();
    if duration_ms == 0 {
        return;
    }

    // The control is held only for the transition duration currently applied to its revealer
    button.set_sensitive(false);
    let button_enable = button.clone();
    gtk::glib::timeout_add_local_once(Duration::from_millis(u64::from(duration_ms)), move || {
        button_enable.set_sensitive(true);
    });
}

fn apply_search_open_state(
    search_revealer: &gtk::Revealer,
    search_entry: &gtk::SearchEntry,
    open: bool,
) {
    search_revealer.set_reveal_child(open);
    if !open && !search_entry.text().is_empty() {
        // Closing search restores the full notification list
        search_entry.set_text("");
    }
}

#[cfg(test)]
#[path = "tests/search.rs"]
mod construction_tests;

#[cfg(test)]
#[path = "tests/search_signals.rs"]
mod signal_tests;
