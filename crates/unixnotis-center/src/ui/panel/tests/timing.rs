use super::{CONTROL_CLICK_GUARD_MS, WIDGETS_TOGGLE_COALESCE_MS};

#[test]
fn startup_timing_keeps_click_guard_above_event_coalescing() {
    let click_guard_ms = std::hint::black_box(CONTROL_CLICK_GUARD_MS);
    let coalesce_ms = std::hint::black_box(WIDGETS_TOGGLE_COALESCE_MS);

    assert!(click_guard_ms > coalesce_ms);
    assert!(coalesce_ms > 0);
}
