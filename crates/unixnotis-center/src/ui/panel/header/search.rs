//! Panel search construction, filtering, and reveal wiring

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use async_channel::TrySendError;
use gtk::prelude::*;
use unixnotis_core::{css::hooks, PanelConfig};

use crate::control::UiEvent;
use crate::ui::panel::behavior::input::{ClickCooldown, LatestBoolEventGate};
use crate::ui::panel::{PanelWidgets, WIDGET_REVEAL_TRANSITION_MS};

pub const SEARCH_REVEAL_TRANSITION_MS: u64 = 180;
const WIDGETS_TOGGLE_COALESCE_MS: u64 = 16;

pub(in crate::ui) struct PanelSearchWidgets {
    pub(in crate::ui) revealer: gtk::Revealer,
    pub(in crate::ui) entry: gtk::SearchEntry,
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

    let search_entry = gtk::SearchEntry::new();
    search_entry.add_css_class(hooks::panel_shell::SEARCH);
    // Placeholder text keeps the intent obvious before the first query
    search_entry.set_placeholder_text(Some(&config.search_placeholder));
    search_entry.set_hexpand(true);
    search_entry.set_tooltip_text(Some("Type to filter notifications"));
    search_shell.append(&leading_accent);
    search_shell.append(&search_entry);
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
    }
}

pub(in crate::ui) fn connect_widget_collapse_toggle(
    panel: &PanelWidgets,
    event_tx: async_channel::Sender<UiEvent>,
) {
    let collapse_gate = LatestBoolEventGate::new(Duration::from_millis(WIDGETS_TOGGLE_COALESCE_MS));
    let collapse_click_gate =
        ClickCooldown::new(Duration::from_millis(WIDGET_REVEAL_TRANSITION_MS));
    let accepted_collapsed = Rc::new(Cell::new(false));
    // Restore guard prevents a rejected click rollback from re-entering this handler
    let collapse_restore = Rc::new(Cell::new(false));

    panel
        .header
        .actions
        .focus_toggle
        .connect_toggled(move |button| {
            if collapse_restore.replace(false) {
                return;
            }

            let collapsed = button.is_active();
            // Ignore clicks while the previous reveal animation is still changing layout
            if !collapse_click_gate.try_start() {
                let accepted = accepted_collapsed.get();
                if collapsed != accepted {
                    // Roll back only the rejected edge so the UI mirrors the running transition
                    collapse_restore.set(true);
                    button.set_active(accepted);
                }
                return;
            }

            accepted_collapsed.set(collapsed);
            // Disable the control until GTK finishes the matching reveal transition
            button.set_sensitive(false);
            let button_enable = button.clone();
            gtk::glib::timeout_add_local_once(
                Duration::from_millis(WIDGET_REVEAL_TRANSITION_MS),
                move || {
                    button_enable.set_sensitive(true);
                },
            );
            collapse_gate.request_widgets_collapsed(&event_tx, collapsed);
        });
}

pub(in crate::ui) fn connect_filter_entry(
    panel: &PanelWidgets,
    event_tx: async_channel::Sender<UiEvent>,
) {
    // SearchChanged covers typing, clear actions, and programmatic text resets
    panel
        .header
        .search
        .entry
        .connect_search_changed(move |entry| {
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

pub(in crate::ui) fn connect_search_toggle(
    panel: &PanelWidgets,
    search_toggle_guard: Rc<Cell<bool>>,
) {
    let search_revealer = panel.header.search.revealer.clone();
    let search_entry = panel.header.search.entry.clone();
    let search_click_gate = ClickCooldown::new(Duration::from_millis(SEARCH_REVEAL_TRANSITION_MS));
    let accepted_search_reveal = Rc::new(Cell::new(false));
    // Programmatic rollback must not be mistaken for a fresh user click
    let search_restore = Rc::new(Cell::new(false));

    panel
        .header
        .actions
        .search_toggle
        .connect_toggled(move |button| {
            if search_toggle_guard.get() || search_restore.replace(false) {
                return;
            }

            let reveal = button.is_active();
            if !search_click_gate.try_start() {
                let accepted = accepted_search_reveal.get();
                if reveal != accepted {
                    // Keep the visual toggle synced with the accepted revealer state
                    search_restore.set(true);
                    button.set_active(accepted);
                }
                return;
            }

            accepted_search_reveal.set(reveal);
            // Freeze the toggle while its revealer animates to the accepted state
            button.set_sensitive(false);
            let button_enable = button.clone();
            gtk::glib::timeout_add_local_once(
                Duration::from_millis(SEARCH_REVEAL_TRANSITION_MS),
                move || {
                    button_enable.set_sensitive(true);
                },
            );
            search_revealer.set_reveal_child(reveal);
            if reveal {
                // Selecting existing text makes the next query replace it immediately
                search_entry.grab_focus();
                search_entry.select_region(0, -1);
            } else if !search_entry.text().is_empty() {
                // Closing search restores the full notification list
                search_entry.set_text("");
            }
        });
}

#[cfg(test)]
#[path = "tests/search.rs"]
mod construction_tests;

#[cfg(test)]
#[path = "tests/search_signals.rs"]
mod signal_tests;
