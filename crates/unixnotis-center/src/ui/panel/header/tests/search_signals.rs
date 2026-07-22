use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gtk::prelude::*;

use super::{
    connect_filter_entry, connect_search_toggle, connect_widget_collapse_toggle, send_filter_event,
    set_search_open,
};
use crate::control::UiEvent;

#[test]
fn filter_event_sends_exact_query_without_waiting() {
    let (event_tx, event_rx) = async_channel::bounded(1);

    send_filter_event(&event_tx, "terminal".to_string());

    let event = event_rx.try_recv().expect("filter event should be queued");
    assert!(matches!(event, UiEvent::FilterChanged(query) if query == "terminal"));
}

#[test]
fn filter_event_ignores_closed_channel() {
    let (event_tx, event_rx) = async_channel::bounded(1);
    drop(event_rx);

    send_filter_event(&event_tx, "ignored".to_string());
}

#[gtk::test]
fn stop_search_closes_revealer_and_clears_filter_immediately() {
    let toggle = gtk::ToggleButton::new();
    let revealer = gtk::Revealer::new();
    let entry = gtk::SearchEntry::new();
    revealer.set_child(Some(&entry));
    let (event_tx, event_rx) = async_channel::bounded(4);
    connect_filter_entry(&entry, event_tx);
    connect_search_toggle(&toggle, &revealer, &entry, Rc::new(Cell::new(false)));

    toggle.set_active(true);
    entry.set_text("urgent");
    assert_eq!(next_filter(&event_rx), "urgent");

    entry.emit_stop_search();

    assert!(!toggle.is_active());
    assert!(!revealer.reveals_child());
    assert!(entry.text().is_empty());
    assert_eq!(next_filter(&event_rx), "");
}

#[gtk::test]
fn guarded_search_toggle_synchronizes_revealer_state() {
    let toggle = gtk::ToggleButton::new();
    let revealer = gtk::Revealer::new();
    let entry = gtk::SearchEntry::new();
    let guard = Rc::new(Cell::new(true));
    connect_search_toggle(&toggle, &revealer, &entry, guard);

    toggle.set_active(true);

    assert!(toggle.is_active());
    assert!(revealer.reveals_child());

    entry.set_text("urgent");
    toggle.set_active(false);

    assert!(!toggle.is_active());
    assert!(!revealer.reveals_child());
    assert!(entry.text().is_empty());
}

#[gtk::test]
fn rapid_search_toggle_restores_the_last_accepted_state() {
    let toggle = gtk::ToggleButton::new();
    let revealer = gtk::Revealer::new();
    let entry = gtk::SearchEntry::new();
    entry.set_text("urgent");
    connect_search_toggle(&toggle, &revealer, &entry, Rc::new(Cell::new(false)));

    toggle.set_active(true);
    assert_eq!(entry.selection_bounds(), Some((0, 6)));
    toggle.set_active(false);

    assert!(toggle.is_active());
    assert!(revealer.reveals_child());
    assert_eq!(entry.text(), "urgent");
}

#[gtk::test]
fn programmatic_panel_close_keeps_search_closed_for_the_next_open() {
    let toggle = gtk::ToggleButton::new();
    let revealer = gtk::Revealer::new();
    let entry = gtk::SearchEntry::new();
    let guard = Rc::new(Cell::new(false));
    connect_search_toggle(&toggle, &revealer, &entry, guard.clone());

    toggle.set_active(true);
    entry.set_text("urgent");
    set_search_open(&toggle, &revealer, &entry, guard.as_ref(), false);

    // Reopening the panel does not mutate search state
    assert!(!toggle.is_active());
    assert!(!revealer.reveals_child());
    assert!(entry.text().is_empty());
}

#[gtk::test]
fn programmatic_search_sync_preserves_an_outer_guard_scope() {
    let toggle = gtk::ToggleButton::new();
    let revealer = gtk::Revealer::new();
    let entry = gtk::SearchEntry::new();
    let guard = Cell::new(true);

    set_search_open(&toggle, &revealer, &entry, &guard, true);

    assert!(guard.get());
    assert!(toggle.is_active());
    assert!(revealer.reveals_child());
}

#[gtk::test]
fn stop_search_closes_a_preexisting_toggle_revealer_mismatch() {
    let toggle = gtk::ToggleButton::new();
    let revealer = gtk::Revealer::new();
    let entry = gtk::SearchEntry::new();
    revealer.set_reveal_child(true);
    entry.set_text("urgent");
    connect_search_toggle(&toggle, &revealer, &entry, Rc::new(Cell::new(false)));

    entry.emit_stop_search();

    assert!(!toggle.is_active());
    assert!(!revealer.reveals_child());
    assert!(entry.text().is_empty());
}

#[gtk::test]
fn widget_collapse_toggle_sends_the_accepted_state_and_rejects_a_burst() {
    let toggle = gtk::ToggleButton::new();
    let revealer = gtk::Revealer::new();
    revealer.set_transition_duration(180);
    let (event_tx, event_rx) = async_channel::bounded(2);
    connect_widget_collapse_toggle(&toggle, &revealer, event_tx);

    toggle.set_active(true);
    assert!(!toggle.is_sensitive());
    toggle.set_active(false);

    // The rejected edge rolls back immediately to the accepted visual state
    assert!(toggle.is_active());
    assert!(next_widgets_collapsed(&event_rx));
}

#[gtk::test]
fn reduced_motion_search_toggle_accepts_an_immediate_reversal() {
    let toggle = gtk::ToggleButton::new();
    let revealer = gtk::Revealer::new();
    revealer.set_transition_duration(0);
    let entry = gtk::SearchEntry::new();
    connect_search_toggle(&toggle, &revealer, &entry, Rc::new(Cell::new(false)));

    toggle.set_active(true);
    toggle.set_active(false);

    assert!(!toggle.is_active());
    assert!(!revealer.reveals_child());
    assert!(toggle.is_sensitive());
}

#[gtk::test]
fn reduced_motion_widget_toggle_accepts_the_latest_state_without_a_cooldown() {
    let toggle = gtk::ToggleButton::new();
    let revealer = gtk::Revealer::new();
    revealer.set_transition_duration(0);
    let (event_tx, event_rx) = async_channel::bounded(2);
    connect_widget_collapse_toggle(&toggle, &revealer, event_tx);

    toggle.set_active(true);
    toggle.set_active(false);

    assert!(!toggle.is_active());
    assert!(toggle.is_sensitive());
    assert!(!next_widgets_collapsed(&event_rx));
}

fn next_filter(event_rx: &async_channel::Receiver<UiEvent>) -> String {
    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        if let Ok(UiEvent::FilterChanged(filter)) = event_rx.try_recv() {
            return filter;
        }
        let context = gtk::glib::MainContext::default();
        while context.pending() {
            context.iteration(false);
        }
        assert!(
            Instant::now() < deadline,
            "search filter event should arrive before timeout"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn next_widgets_collapsed(event_rx: &async_channel::Receiver<UiEvent>) -> bool {
    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        if let Ok(UiEvent::WidgetsCollapsed(collapsed)) = event_rx.try_recv() {
            return collapsed;
        }
        let context = gtk::glib::MainContext::default();
        while context.pending() {
            context.iteration(false);
        }
        assert!(
            Instant::now() < deadline,
            "widget collapse event should arrive before timeout"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
}
