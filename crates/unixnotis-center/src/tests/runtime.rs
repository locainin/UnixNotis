use super::ReloadGate;
use crate::dbus;
use std::sync::{Arc, Barrier};

fn queued_event(receiver: &async_channel::Receiver<dbus::UiEvent>) -> dbus::UiEvent {
    receiver.try_recv().expect("reload event should be queued")
}

#[test]
fn repeated_reload_requests_coalesce_until_processing_completes() {
    let gate = ReloadGate::new();
    let (sender, receiver) = async_channel::bounded(2);

    assert!(!gate.request_css(&sender));
    assert!(!gate.request_css(&sender));
    assert_eq!(receiver.len(), 1);
    assert!(matches!(queued_event(&receiver), dbus::UiEvent::CssReload));

    assert!(!gate.complete_css(&sender));
    assert!(matches!(queued_event(&receiver), dbus::UiEvent::CssReload));
    assert!(!gate.complete_css(&sender));
    assert!(receiver.is_empty());
}

#[test]
fn full_queue_reload_retries_once_space_is_available() {
    let gate = ReloadGate::new();
    let (sender, receiver) = async_channel::bounded(1);

    assert!(!gate.request_css(&sender));
    assert!(gate.request_config(&sender));
    assert!(gate.has_pending());
    let _css = queued_event(&receiver);

    gate.flush(&sender);

    assert!(matches!(
        queued_event(&receiver),
        dbus::UiEvent::ConfigReload
    ));
    assert!(!gate.has_pending());
    assert!(!gate.complete_config(&sender));
}

#[test]
fn css_and_config_reload_slots_are_independent() {
    let gate = ReloadGate::new();
    let (sender, receiver) = async_channel::bounded(2);

    assert!(!gate.request_css(&sender));
    assert!(!gate.request_config(&sender));
    assert_eq!(receiver.len(), 2);
}

#[test]
fn concurrent_request_and_completion_never_strand_a_reload() {
    for _attempt in 0..1_000 {
        let gate = Arc::new(ReloadGate::new());
        let (sender, receiver) = async_channel::bounded(2);
        assert!(!gate.request_css(&sender));
        let _initial = queued_event(&receiver);
        let barrier = Arc::new(Barrier::new(2));

        std::thread::scope(|scope| {
            let request_gate = Arc::clone(&gate);
            let request_sender = sender.clone();
            let request_barrier = Arc::clone(&barrier);
            scope.spawn(move || {
                request_barrier.wait();
                let _needs_retry = request_gate.request_css(&request_sender);
            });
            barrier.wait();
            let _needs_retry = gate.complete_css(&sender);
        });

        assert!(matches!(queued_event(&receiver), dbus::UiEvent::CssReload));
        assert!(receiver.is_empty());
    }
}
