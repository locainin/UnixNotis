//! Search, filter, and widget-collapse wiring

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use async_channel::TrySendError;
use gtk::prelude::*;

use super::super::input_guard::{ClickCooldown, LatestBoolEventGate};
use super::super::panel;
use super::timing::WIDGETS_TOGGLE_COALESCE_MS;
use crate::dbus::UiEvent;

pub(super) fn connect_widget_collapse_toggle(
    panel: &panel::PanelWidgets,
    event_tx: async_channel::Sender<UiEvent>,
) {
    let collapse_gate = LatestBoolEventGate::new(Duration::from_millis(WIDGETS_TOGGLE_COALESCE_MS));
    let collapse_click_gate =
        ClickCooldown::new(Duration::from_millis(panel::WIDGET_REVEAL_TRANSITION_MS));
    let accepted_collapsed = Rc::new(Cell::new(false));
    let collapse_restore = Rc::new(Cell::new(false));

    panel.focus_toggle.connect_toggled(move |button| {
        if collapse_restore.replace(false) {
            return;
        }

        let collapsed = button.is_active();
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
        button.set_sensitive(false);
        let button_enable = button.clone();
        gtk::glib::timeout_add_local_once(
            Duration::from_millis(panel::WIDGET_REVEAL_TRANSITION_MS),
            move || {
                button_enable.set_sensitive(true);
            },
        );
        collapse_gate.request_widgets_collapsed(&event_tx, collapsed);
    });
}

pub(super) fn connect_filter_entry(
    panel: &panel::PanelWidgets,
    event_tx: async_channel::Sender<UiEvent>,
) {
    panel.search_entry.connect_search_changed(move |entry| {
        let event = UiEvent::FilterChanged(entry.text().to_string());
        match event_tx.try_send(event) {
            Ok(()) => {}
            Err(TrySendError::Full(event)) => {
                // Search changes are small and should retry instead of disappearing under bursts
                let event_tx = event_tx.clone();
                gtk::glib::MainContext::default().spawn_local(async move {
                    let _ = event_tx.send(event).await;
                });
            }
            Err(TrySendError::Closed(_)) => {}
        }
    });
}

pub(super) fn connect_search_toggle(
    panel: &panel::PanelWidgets,
    search_toggle_guard: Rc<Cell<bool>>,
) {
    let search_revealer = panel.search_revealer.clone();
    let search_entry = panel.search_entry.clone();
    let search_click_gate =
        ClickCooldown::new(Duration::from_millis(panel::SEARCH_REVEAL_TRANSITION_MS));
    let accepted_search_reveal = Rc::new(Cell::new(false));
    let search_restore = Rc::new(Cell::new(false));

    panel.search_toggle.connect_toggled(move |button| {
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
        button.set_sensitive(false);
        let button_enable = button.clone();
        gtk::glib::timeout_add_local_once(
            Duration::from_millis(panel::SEARCH_REVEAL_TRANSITION_MS),
            move || {
                button_enable.set_sensitive(true);
            },
        );
        search_revealer.set_reveal_child(reveal);
        if reveal {
            search_entry.grab_focus();
            search_entry.select_region(0, -1);
        } else if !search_entry.text().is_empty() {
            search_entry.set_text("");
        }
    });
}
