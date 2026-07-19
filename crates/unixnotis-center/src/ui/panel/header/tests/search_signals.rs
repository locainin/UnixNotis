use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gtk::prelude::*;

use super::{
    connect_filter_entry, connect_search_toggle, connect_widget_collapse_toggle, send_filter_event,
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
fn guarded_search_toggle_does_not_change_revealer_state() {
    let toggle = gtk::ToggleButton::new();
    let revealer = gtk::Revealer::new();
    let entry = gtk::SearchEntry::new();
    let guard = Rc::new(Cell::new(true));
    connect_search_toggle(&toggle, &revealer, &entry, guard);

    toggle.set_active(true);

    assert!(toggle.is_active());
    assert!(!revealer.reveals_child());
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
fn widget_collapse_toggle_sends_the_accepted_state_and_rejects_a_burst() {
    let toggle = gtk::ToggleButton::new();
    let (event_tx, event_rx) = async_channel::bounded(2);
    connect_widget_collapse_toggle(&toggle, event_tx);

    toggle.set_active(true);
    assert!(!toggle.is_sensitive());
    toggle.set_active(false);

    // The rejected edge rolls back immediately to the accepted visual state
    assert!(toggle.is_active());
    assert!(next_widgets_collapsed(&event_rx));
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
