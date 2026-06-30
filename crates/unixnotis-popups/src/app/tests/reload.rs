use crate::app::reload::ReloadGate;
use crate::dbus;
use std::sync::{Arc, Mutex};

fn queued_event(rx: &async_channel::Receiver<dbus::UiEvent>) -> dbus::UiEvent {
    // Reload sends are synchronous try_send calls, so try_recv keeps mutation
    // failures from hanging until cargo-mutants kills the whole test binary
    rx.try_recv().expect("reload event should be queued now")
}

#[test]
fn reload_gate_retries_when_queue_is_full() {
    let gate = ReloadGate::new();
    let (tx, rx) = async_channel::bounded(1);

    assert!(!gate.request_css(&tx));
    assert!(!gate.has_pending());

    assert!(gate.request_config(&tx));
    assert!(gate.has_pending());

    let _ = queued_event(&rx);
    gate.flush(&tx);
    assert!(!gate.has_pending());
}

#[test]
fn reload_gate_has_pending_only_tracks_blocked_retry_work() {
    let gate = ReloadGate::new();
    let (tx, rx) = async_channel::bounded(1);

    // A queued reload is already represented, so has_pending stays false here
    assert!(!gate.request_css(&tx));
    assert!(!gate.has_pending());

    // A second reload kind that cannot enter the full queue is true pending retry work
    assert!(gate.request_config(&tx));
    assert!(gate.has_pending());

    let _ = queued_event(&rx);
    gate.flush(&tx);
    let queued = queued_event(&rx);
    assert!(matches!(queued, dbus::UiEvent::ConfigReload));
    assert!(!gate.has_pending());
}

#[test]
fn reload_gate_keeps_trailing_reload_when_request_arrives_during_handling() {
    let gate = ReloadGate::new();
    let (tx, rx) = async_channel::bounded(1);

    assert!(!gate.request_css(&tx));
    let _ = queued_event(&rx);

    // Another CSS watcher hit landed while the first reload was still being handled
    assert!(!gate.request_css(&tx));
    assert!(!gate.complete_css(&tx));

    let queued = queued_event(&rx);
    assert!(matches!(queued, dbus::UiEvent::CssReload));
    assert!(!gate.complete_css(&tx));
    assert!(!gate.has_pending());
}

#[test]
fn reload_gate_does_not_queue_extra_reload_after_retry_covers_latest_state() {
    let gate = ReloadGate::new();
    let (tx, rx) = async_channel::bounded(1);

    assert!(!gate.request_css(&tx));
    assert!(gate.request_config(&tx));
    // The later config change should be covered by the retried config reload
    assert!(!gate.request_config(&tx));

    let _ = queued_event(&rx);
    gate.flush(&tx);

    let queued = queued_event(&rx);
    assert!(matches!(queued, dbus::UiEvent::ConfigReload));
    assert!(!gate.complete_config(&tx));
    assert!(rx.is_empty());
    assert!(!gate.has_pending());
}

#[test]
fn reload_gate_clears_state_when_queue_is_closed() {
    let gate = ReloadGate::new();
    let (tx, rx) = async_channel::bounded(1);
    drop(rx);

    // Closed queues should clear the slot instead of leaving a stuck represented reload
    assert!(!gate.request_css(&tx));
    assert!(!gate.has_pending());

    let (tx, rx) = async_channel::bounded(1);
    // This catches stale state before a completion path has a chance to clean it later
    assert!(!gate.request_css(&tx));
    assert!(matches!(queued_event(&rx), dbus::UiEvent::CssReload));

    assert!(!gate.complete_css(&tx));
    assert!(!gate.has_pending());
}

#[test]
fn reload_gate_tracks_css_and_config_independently() {
    let gate = ReloadGate::new();
    let (tx, rx) = async_channel::bounded(2);

    // A running CSS reload must not block config reloads from being represented too
    assert!(!gate.request_css(&tx));
    assert!(!gate.request_config(&tx));
    assert!(!gate.has_pending());

    let first = queued_event(&rx);
    let second = queued_event(&rx);
    assert!(matches!(first, dbus::UiEvent::CssReload));
    assert!(matches!(second, dbus::UiEvent::ConfigReload));

    assert!(!gate.complete_css(&tx));
    assert!(!gate.complete_config(&tx));
    assert!(!gate.has_pending());
}

#[test]
fn reload_gate_keeps_retry_pending_when_new_change_arrives_before_flush() {
    let gate = ReloadGate::new();
    let (tx, rx) = async_channel::bounded(1);

    assert!(!gate.request_css(&tx));
    assert!(gate.request_config(&tx));
    // Another config watcher hit landed while the config reload was still waiting for room
    assert!(!gate.request_config(&tx));
    assert!(gate.has_pending());

    let _ = queued_event(&rx);
    gate.flush(&tx);

    let queued = queued_event(&rx);
    assert!(matches!(queued, dbus::UiEvent::ConfigReload));
    assert!(!gate.complete_config(&tx));
    assert!(!gate.has_pending());
}

#[test]
fn reload_gate_complete_config_reports_retry_when_trailing_send_is_blocked() {
    let gate = ReloadGate::new();
    let (tx, rx) = async_channel::bounded(1);

    assert!(!gate.request_config(&tx));
    assert!(matches!(queued_event(&rx), dbus::UiEvent::ConfigReload));

    // The next config watcher hit should be represented after the current reload finishes
    assert!(!gate.request_config(&tx));
    assert!(!gate.request_css(&tx));

    // The CSS reload occupies the single queue slot, so config completion must ask the timer to retry
    assert!(gate.complete_config(&tx));
    assert!(gate.has_pending());

    assert!(matches!(queued_event(&rx), dbus::UiEvent::CssReload));
    gate.flush(&tx);
    assert!(matches!(queued_event(&rx), dbus::UiEvent::ConfigReload));
    assert!(!gate.has_pending());
}

#[test]
fn start_reload_timer_registers_only_one_timer_source() {
    let gate = Arc::new(ReloadGate::new());
    let (tx, _rx) = async_channel::bounded(1);
    let timer_state = Arc::new(Mutex::new(None));

    super::super::reload::start_reload_timer(&gate, &tx, &timer_state);
    super::super::reload::start_reload_timer(&gate, &tx, &timer_state);

    let source_id = timer_state
        .lock()
        .expect("timer state lock")
        .take()
        .expect("timer source should be registered");
    // The source is not meant to run in this unit test; removing it keeps the GLib context tidy
    source_id.remove();
}
