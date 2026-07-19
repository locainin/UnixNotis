use std::cell::Cell;
use std::rc::Rc;

use chrono::NaiveDate;

use super::{countdown_control_flow, format_dnd_remaining, tomorrow_date, DndCountdown};

#[test]
fn remaining_time_is_hidden_after_expiry_and_rounded_up_before_it() {
    assert_eq!(format_dnd_remaining(100, 100), "");
    assert_eq!(format_dnd_remaining(99, 100), "");
    assert_eq!(format_dnd_remaining(101, 100), "· 1m");
    assert_eq!(format_dnd_remaining(100 + 47 * 60, 100), "· 47m");
}

#[test]
fn remaining_time_keeps_hours_compact_without_losing_partial_hour() {
    assert_eq!(format_dnd_remaining(100 + 60 * 60, 100), "· 1h");
    assert_eq!(
        format_dnd_remaining(100 + 2 * 60 * 60 + 5 * 60, 100),
        "· 2h 5m"
    );
}

#[test]
fn morning_choice_uses_the_next_local_eight_oclock() {
    let today = NaiveDate::from_ymd_opt(2026, 7, 18).expect("valid date");

    assert_eq!(tomorrow_date(today), NaiveDate::from_ymd_opt(2026, 7, 19));
}

#[test]
fn countdown_stops_at_the_deadline_and_continues_only_while_future() {
    assert_eq!(
        countdown_control_flow(100, 99),
        gtk::glib::ControlFlow::Continue
    );
    assert_eq!(
        countdown_control_flow(100, 100),
        gtk::glib::ControlFlow::Break
    );
    assert_eq!(
        countdown_control_flow(100, 101),
        gtk::glib::ControlFlow::Break
    );
}

#[gtk::test]
fn dropping_countdown_removes_its_live_source() {
    let callback_runs = Rc::new(Cell::new(0));
    let countdown = test_countdown(callback_runs.clone());

    drop(countdown);
    drain_main_context();

    assert_eq!(callback_runs.get(), 0);
}

fn test_countdown(callback_runs: Rc<Cell<u32>>) -> DndCountdown {
    let source = gtk::glib::idle_add_local(move || {
        callback_runs.set(callback_runs.get() + 1);
        gtk::glib::ControlFlow::Break
    });
    DndCountdown {
        source: Some(source),
        active: Rc::new(Cell::new(true)),
    }
}

fn drain_main_context() {
    let context = gtk::glib::MainContext::default();
    while context.pending() {
        context.iteration(false);
    }
}
