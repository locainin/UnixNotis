//! Shared motion-policy tests

use super::apply_revealer_preference;

#[gtk::test]
fn reduced_motion_makes_revealer_transitions_immediate_and_restorable() {
    let revealer = gtk::Revealer::new();

    apply_revealer_preference(&revealer, 180, true);
    assert_eq!(revealer.transition_duration(), 0);

    apply_revealer_preference(&revealer, 180, false);
    assert_eq!(revealer.transition_duration(), 180);
}
