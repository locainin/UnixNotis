use super::{start_reload_timer, ReloadGate};
use crate::dbus;
use proptest::prelude::*;
use proptest::test_runner::RngSeed;
use std::sync::{Arc, Barrier, Mutex};

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
fn retry_without_a_trailing_change_does_not_create_an_extra_reload() {
    let gate = ReloadGate::new();
    let (sender, receiver) = async_channel::bounded(1);

    assert!(!gate.request_css(&sender));
    assert!(gate.request_config(&sender));
    // A flush while still full must preserve only the original pending config reload
    gate.flush(&sender);
    assert!(matches!(queued_event(&receiver), dbus::UiEvent::CssReload));
    gate.flush(&sender);
    assert!(matches!(
        queued_event(&receiver),
        dbus::UiEvent::ConfigReload
    ));

    assert!(!gate.complete_config(&sender));
    assert!(receiver.is_empty());
}

#[test]
fn config_completion_reports_when_a_trailing_reload_needs_retry() {
    let gate = ReloadGate::new();
    let (sender, receiver) = async_channel::bounded(1);

    assert!(!gate.request_config(&sender));
    assert!(!gate.request_config(&sender));

    assert!(gate.complete_config(&sender));
    assert!(gate.has_pending());
    assert!(matches!(
        queued_event(&receiver),
        dbus::UiEvent::ConfigReload
    ));
}

#[test]
fn reload_timer_registration_records_the_active_source() {
    let gate = Arc::new(ReloadGate::new());
    let (sender, _receiver) = async_channel::bounded(1);
    let timer_state = Arc::new(Mutex::new(None));

    start_reload_timer(&gate, &sender, &timer_state);

    let source = timer_state
        .lock()
        .expect("lock reload timer state")
        .take()
        .expect("reload timer should be registered");
    source.remove();
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

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 256,
        rng_seed: RngSeed::Fixed(0x4345_4e54_4552_4741),
        ..ProptestConfig::default()
    })]

    #[test]
    fn arbitrary_reload_sequences_settle_without_stuck_retry_state(
        operations in prop::collection::vec(0_u8..5, 0..=100),
    ) {
        let gate = ReloadGate::new();
        let (sender, receiver) = async_channel::bounded(2);

        for operation in operations {
            match operation {
                0 => { let _needs_retry = gate.request_css(&sender); }
                1 => { let _needs_retry = gate.request_config(&sender); }
                2 => gate.flush(&sender),
                3 | 4 => complete_one(&gate, &sender, &receiver),
                _ => unreachable!("operation generator stays inside range"),
            }
        }

        for _ in 0..16 {
            gate.flush(&sender);
            if receiver.is_empty() && !gate.has_pending() {
                break;
            }
            complete_one(&gate, &sender, &receiver);
        }
        while !receiver.is_empty() {
            complete_one(&gate, &sender, &receiver);
        }

        prop_assert!(!gate.has_pending());
        prop_assert!(receiver.is_empty());
    }
}

fn complete_one(
    gate: &ReloadGate,
    sender: &async_channel::Sender<dbus::UiEvent>,
    receiver: &async_channel::Receiver<dbus::UiEvent>,
) {
    let Ok(event) = receiver.try_recv() else {
        return;
    };
    match event {
        dbus::UiEvent::CssReload => {
            let _needs_retry = gate.complete_css(sender);
        }
        dbus::UiEvent::ConfigReload => {
            let _needs_retry = gate.complete_config(sender);
        }
        _ => unreachable!("reload gate emits only reload events"),
    }
}
