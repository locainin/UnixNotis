use super::{
    marquee_can_start, marquee_should_stop, marquee_should_tick, MarqueeLabel, MarqueeState,
};

fn ready_marquee_state() -> MarqueeState {
    MarqueeState {
        overflows: true,
        is_mapped: true,
        ..MarqueeState::default()
    }
}

#[test]
fn marquee_start_policy_requires_one_idle_visible_overflow() {
    let mut state = ready_marquee_state();
    assert!(marquee_can_start(&state));

    state.is_ticking = true;
    assert!(!marquee_can_start(&state));
    state.is_ticking = false;

    state.reduced_motion = true;
    assert!(!marquee_can_start(&state));
    state.reduced_motion = false;

    state.overflows = false;
    assert!(!marquee_can_start(&state));
    state.overflows = true;

    state.is_mapped = false;
    assert!(!marquee_can_start(&state));
}

#[test]
fn marquee_stop_policy_covers_every_inactive_state() {
    let mut state = ready_marquee_state();
    assert!(!marquee_should_stop(&state));

    state.overflows = false;
    assert!(marquee_should_stop(&state));
    state.overflows = true;

    state.reduced_motion = true;
    assert!(marquee_should_stop(&state));
    state.reduced_motion = false;

    state.is_mapped = false;
    assert!(marquee_should_stop(&state));
}

#[test]
fn marquee_starts_when_short_title_exceeds_pixel_budget() {
    assert!(marquee_should_tick(32, 17, 112, 81));
}

#[test]
fn marquee_starts_when_character_limit_is_exceeded() {
    assert!(marquee_should_tick(16, 17, 70, 100));
}

#[test]
fn marquee_stays_idle_when_text_fits_both_limits() {
    assert!(!marquee_should_tick(32, 17, 80, 81));
    assert!(!marquee_should_tick(17, 17, 80, 81));
    assert!(!marquee_should_tick(32, 17, 81, 81));
}

#[test]
fn disabled_marquee_never_starts_for_overflowing_text() {
    assert!(!marquee_should_tick(0, 40, 300, 81));
}

#[gtk::test]
fn reduced_motion_keeps_overflowing_text_stable_without_a_timer() {
    let marquee = MarqueeLabel::new("test-marquee", 40, 4);
    marquee.state.borrow_mut().is_mapped = true;
    marquee.set_reduced_motion(true);

    marquee.set_text("Long title");

    let state = marquee.state.borrow();
    assert!(state.overflows);
    assert!(state.reduced_motion);
    assert!(!state.is_ticking);
    assert!(state.tick_source.is_none());
    assert_eq!(marquee.label.text(), "Long title");
}

#[gtk::test]
fn runtime_reduced_motion_cancels_and_restores_one_marquee_source() {
    let marquee = MarqueeLabel::new("test-marquee", 40, 4);
    marquee.state.borrow_mut().is_mapped = true;
    marquee.set_text("Long title");
    assert!(marquee.state.borrow().tick_source.is_some());

    marquee.set_reduced_motion(true);
    {
        let state = marquee.state.borrow();
        assert!(!state.is_ticking);
        assert!(state.tick_source.is_none());
        assert!(state.offset.abs() <= f64::EPSILON);
    }
    assert_eq!(marquee.label.text(), "Long title");

    marquee.set_reduced_motion(false);
    let restarted_source = marquee
        .state
        .borrow()
        .tick_source
        .as_ref()
        .expect("overflow should restart one source")
        .as_raw();
    marquee.set_reduced_motion(false);
    assert_eq!(
        marquee
            .state
            .borrow()
            .tick_source
            .as_ref()
            .expect("repeated preference should retain the source")
            .as_raw(),
        restarted_source
    );

    // Removing the source keeps it from escaping the test main context
    marquee.set_reduced_motion(true);
}

#[gtk::test]
fn disabling_reduced_motion_does_not_start_a_timer_when_text_fits() {
    let marquee = MarqueeLabel::new("test-marquee", 400, 32);
    marquee.state.borrow_mut().is_mapped = true;
    marquee.set_reduced_motion(true);
    marquee.set_text("Short title");

    marquee.set_reduced_motion(false);

    let state = marquee.state.borrow();
    assert!(!state.overflows);
    assert!(!state.is_ticking);
    assert!(state.tick_source.is_none());
}

#[gtk::test]
fn replacing_overflow_with_short_text_cancels_the_active_source() {
    let marquee = MarqueeLabel::new("test-marquee", 40, 4);
    marquee.state.borrow_mut().is_mapped = true;
    marquee.set_text("Long title");
    assert!(marquee.state.borrow().tick_source.is_some());

    marquee.set_text("Fit");

    let state = marquee.state.borrow();
    assert!(!state.overflows);
    assert!(!state.is_ticking);
    assert!(state.tick_source.is_none());
    assert_eq!(marquee.label.text(), "Fit");
}
