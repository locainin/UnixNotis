use crate::app::reload::ReloadGate;
use crate::dbus;

#[test]
fn reload_gate_retries_when_queue_is_full() {
    let gate = ReloadGate::new();
    let (tx, rx) = async_channel::bounded(1);

    assert!(!gate.request_css(&tx));
    assert!(!gate.has_pending());

    assert!(gate.request_config(&tx));
    assert!(gate.has_pending());

    let _ = rx.recv_blocking();
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

    let _ = rx.recv_blocking();
    gate.flush(&tx);
    let queued = rx.recv_blocking().expect("queued pending config reload");
    assert!(matches!(queued, dbus::UiEvent::ConfigReload));
    assert!(!gate.has_pending());
}

#[test]
fn reload_gate_keeps_trailing_reload_when_request_arrives_during_handling() {
    let gate = ReloadGate::new();
    let (tx, rx) = async_channel::bounded(1);

    assert!(!gate.request_css(&tx));
    let _ = rx.recv_blocking();

    // Another CSS watcher hit landed while the first reload was still being handled
    assert!(!gate.request_css(&tx));
    assert!(!gate.complete_css(&tx));

    let queued = rx.recv_blocking().expect("queued trailing css reload");
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

    let _ = rx.recv_blocking();
    gate.flush(&tx);

    let queued = rx.recv_blocking().expect("queued retried config reload");
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

    // Closed queues should clear the slot instead of leaving a stuck pending bit
    assert!(!gate.request_css(&tx));
    assert!(!gate.has_pending());
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

    let first = rx.recv_blocking().expect("first queued reload");
    let second = rx.recv_blocking().expect("second queued reload");
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

    let _ = rx.recv_blocking();
    gate.flush(&tx);

    let queued = rx.recv_blocking().expect("queued pending config reload");
    assert!(matches!(queued, dbus::UiEvent::ConfigReload));
    assert!(!gate.complete_config(&tx));
    assert!(!gate.has_pending());
}
