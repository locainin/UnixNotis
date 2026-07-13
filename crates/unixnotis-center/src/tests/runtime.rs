use super::ReloadGate;
use crate::dbus;

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
