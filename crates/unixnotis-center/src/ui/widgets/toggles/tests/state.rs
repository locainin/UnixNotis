use super::ToggleRefreshGate;

#[test]
fn refresh_gate_queues_one_trailing_refresh() {
    let gate = ToggleRefreshGate::new();

    assert!(gate.begin_or_queue());
    assert!(!gate.begin_or_queue());
    assert!(gate.finish());
}

#[test]
fn refresh_gate_clears_pending_after_finish() {
    let gate = ToggleRefreshGate::new();

    assert!(gate.begin_or_queue());
    assert!(!gate.begin_or_queue());
    assert!(gate.finish());
    assert!(!gate.finish());
    assert!(gate.begin_or_queue());
}

#[test]
fn refresh_gate_does_not_stack_multiple_pending_runs() {
    let gate = ToggleRefreshGate::new();

    assert!(gate.begin_or_queue());
    assert!(!gate.begin_or_queue());
    assert!(!gate.begin_or_queue());
    assert!(gate.finish());
    assert!(!gate.finish());
}
