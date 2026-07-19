use std::time::Duration;

use super::{release_cooldown_if_current, ClickCooldown, LatestBoolEventGate};
use crate::control::UiEvent;

#[gtk::test]
fn click_cooldown_rejects_bursts_and_reopens_after_its_timeout() {
    let guard = ClickCooldown::new(Duration::ZERO);

    assert!(guard.try_start());
    assert!(!guard.try_start());

    let context = gtk::glib::MainContext::default();
    while context.pending() {
        context.iteration(false);
    }
    assert!(guard.try_start());
}

#[gtk::test]
fn click_cooldown_release_accepts_an_immediate_semantic_action() {
    let guard = ClickCooldown::new(Duration::from_secs(1));

    assert!(guard.try_start());
    guard.release();

    assert!(guard.try_start());
}

#[gtk::test]
fn released_timeout_cannot_end_the_next_cooldown_early() {
    let guard = ClickCooldown::new(Duration::ZERO);
    assert!(guard.try_start());
    let retired_ticket = guard.generation.get();

    guard.release();
    assert!(guard.try_start());
    let current_ticket = guard.generation.get();

    release_cooldown_if_current(&guard.blocked, &guard.generation, retired_ticket);
    assert!(!guard.try_start());
    release_cooldown_if_current(&guard.blocked, &guard.generation, current_ticket);
    assert!(guard.try_start());

    // Drain zero-duration sources so this test leaves no main-context work behind
    drain_main_context();
}

#[gtk::test]
fn latest_bool_event_gate_sends_the_requested_state() {
    let gate = LatestBoolEventGate::new(Duration::ZERO);
    let (event_tx, event_rx) = async_channel::bounded(1);

    gate.request_widgets_collapsed(&event_tx, true);
    drain_main_context();

    assert!(matches!(
        event_rx.try_recv(),
        Ok(UiEvent::WidgetsCollapsed(true))
    ));
}

#[gtk::test]
fn latest_bool_event_gate_coalesces_to_the_newest_state() {
    let gate = LatestBoolEventGate::new(Duration::ZERO);
    let (event_tx, event_rx) = async_channel::bounded(1);

    gate.request_widgets_collapsed(&event_tx, true);
    gate.request_widgets_collapsed(&event_tx, false);
    drain_main_context();

    assert!(matches!(
        event_rx.try_recv(),
        Ok(UiEvent::WidgetsCollapsed(false))
    ));
    assert!(event_rx.try_recv().is_err());
}

fn drain_main_context() {
    let context = gtk::glib::MainContext::default();
    while context.pending() {
        context.iteration(false);
    }
}
