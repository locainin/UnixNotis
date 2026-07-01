use super::gate::SliderRefreshGate;

#[test]
fn refresh_gate_queues_one_trailing_refresh() {
    let gate = SliderRefreshGate::new();

    assert!(!gate.is_in_flight());
    assert!(gate.begin_or_queue());
    assert!(gate.is_in_flight());
    assert!(!gate.begin_or_queue());
    assert!(gate.finish());
    assert!(!gate.is_in_flight());
}

#[test]
fn refresh_gate_clears_pending_after_finish() {
    let gate = SliderRefreshGate::new();

    assert!(gate.begin_or_queue());
    assert!(!gate.begin_or_queue());
    assert!(gate.finish());
    assert!(!gate.finish());
    assert!(gate.begin_or_queue());
}

#[test]
fn refresh_gate_does_not_stack_multiple_pending_runs() {
    let gate = SliderRefreshGate::new();

    assert!(gate.begin_or_queue());
    assert!(!gate.begin_or_queue());
    assert!(!gate.begin_or_queue());
    assert!(gate.finish());
    assert!(!gate.finish());
}

#[test]
fn refresh_gate_can_reenter_after_pending_work_is_consumed() {
    let gate = SliderRefreshGate::new();

    assert!(gate.begin_or_queue());
    assert!(!gate.begin_or_queue());
    assert!(gate.finish());
    assert!(gate.begin_or_queue());
    assert!(!gate.begin_or_queue());
    assert!(gate.finish());
}
