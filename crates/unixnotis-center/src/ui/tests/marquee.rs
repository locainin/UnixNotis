use super::marquee_should_tick;

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
