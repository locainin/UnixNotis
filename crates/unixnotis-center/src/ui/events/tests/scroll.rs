use std::cell::Cell;
use std::rc::Rc;

use gtk::prelude::*;

use super::super::{
    reset_notification_scroll, scroll_reset_generation_is_current, should_apply_scroll_reset,
    should_snap_to_top_value, ScrollResetPolicy,
};

#[test]
fn near_top_insertions_snap_to_the_first_row() {
    assert!(should_snap_to_top_value(0.0, 0.0));
    assert!(should_snap_to_top_value(17.5, 0.0));
    assert!(!should_snap_to_top_value(18.1, 0.0));
}

#[test]
fn scroll_threshold_follows_nonzero_adjustment_lower_bound() {
    assert!(should_snap_to_top_value(118.0, 100.0));
    assert!(!should_snap_to_top_value(118.1, 100.0));
}

#[test]
fn stale_scroll_reset_generation_is_rejected() {
    assert!(!scroll_reset_generation_is_current(11, 12));
    assert!(scroll_reset_generation_is_current(12, 12));
}

#[gtk::test]
fn scroll_reset_requires_current_generation_and_near_top_position() {
    let scroller = gtk::ScrolledWindow::new();
    let adjustment = gtk::Adjustment::new(100.0, 100.0, 300.0, 1.0, 10.0, 10.0);
    scroller.set_vadjustment(Some(&adjustment));
    adjustment.set_value(100.0);

    assert!(should_apply_scroll_reset(
        4,
        4,
        2,
        2,
        &scroller,
        ScrollResetPolicy::NearTopOnly,
    ));
    assert!(!should_apply_scroll_reset(
        3,
        4,
        2,
        2,
        &scroller,
        ScrollResetPolicy::NearTopOnly,
    ));

    adjustment.set_value(130.0);
    assert!(!should_apply_scroll_reset(
        4,
        4,
        2,
        2,
        &scroller,
        ScrollResetPolicy::NearTopOnly,
    ));
    assert!(should_apply_scroll_reset(
        4,
        4,
        2,
        2,
        &scroller,
        ScrollResetPolicy::Force,
    ));
}

#[gtk::test]
fn force_scroll_reset_rejects_a_new_user_scroll_generation() {
    let scroller = gtk::ScrolledWindow::new();
    let adjustment = gtk::Adjustment::new(100.0, 100.0, 300.0, 1.0, 10.0, 10.0);
    scroller.set_vadjustment(Some(&adjustment));

    // Adjustment movement alone may come from layout, so only the explicit
    // interaction generation identifies a real user scroll
    assert!(!should_apply_scroll_reset(
        4,
        4,
        3,
        2,
        &scroller,
        ScrollResetPolicy::Force,
    ));
}

#[gtk::test]
fn force_scroll_reset_accepts_layout_adjustment_changes_without_user_input() {
    let scroller = gtk::ScrolledWindow::new();
    let adjustment = gtk::Adjustment::new(130.0, 100.0, 300.0, 1.0, 10.0, 10.0);
    scroller.set_vadjustment(Some(&adjustment));

    assert!(should_apply_scroll_reset(
        4,
        4,
        2,
        2,
        &scroller,
        ScrollResetPolicy::Force,
    ));
}

#[gtk::test]
fn deferred_scroll_reset_updates_the_adjustment_after_idle() {
    let scroller = gtk::ScrolledWindow::new();
    let adjustment = gtk::Adjustment::new(108.0, 100.0, 300.0, 1.0, 10.0, 10.0);
    scroller.set_vadjustment(Some(&adjustment));
    adjustment.set_value(108.0);
    let generation = Rc::new(Cell::new(7));

    let user_generation = Rc::new(Cell::new(3));
    reset_notification_scroll(
        &scroller,
        generation,
        7,
        user_generation,
        3,
        ScrollResetPolicy::NearTopOnly,
    );
    while gtk::glib::MainContext::default().pending() {
        gtk::glib::MainContext::default().iteration(false);
    }

    assert!((adjustment.value() - adjustment.lower()).abs() < f64::EPSILON);
}
