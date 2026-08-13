//! Shared motion-policy tests

use super::{apply_revealer_preference, immediate_reveal_edges};

#[test]
fn immediate_reveal_edges_only_finish_inflight_reduced_motion_transitions() {
    assert_eq!(
        immediate_reveal_edges(true, false, true),
        Some([false, true])
    );
    assert_eq!(
        immediate_reveal_edges(true, true, false),
        Some([true, false])
    );
    assert_eq!(immediate_reveal_edges(true, true, true), None);
    assert_eq!(immediate_reveal_edges(true, false, false), None);
    assert_eq!(immediate_reveal_edges(false, false, true), None);
    assert_eq!(immediate_reveal_edges(false, true, false), None);
}

#[gtk::test]
fn reduced_motion_makes_revealer_transitions_immediate_and_restorable() {
    let revealer = gtk::Revealer::new();

    apply_revealer_preference(&revealer, 180, true);
    assert_eq!(revealer.transition_duration(), 0);

    apply_revealer_preference(&revealer, 180, false);
    assert_eq!(revealer.transition_duration(), 180);
}
