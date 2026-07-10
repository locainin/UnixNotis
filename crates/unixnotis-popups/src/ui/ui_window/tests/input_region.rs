use super::{
    build_popup_input_region, popup_surface_needs_retry, InputRegionSignature, PopupInputRegionState,
};
use gtk::cairo;

#[test]
fn interactive_region_uses_full_surface_bounds() {
    let (_, signature) = build_popup_input_region(320, 180, false, true);

    assert_eq!(signature.reactive_rects.len(), 1);
    assert_eq!(
        signature.reactive_rects[0],
        cairo::RectangleInt::new(0, 0, 320, 180)
    );
}

#[test]
fn click_through_region_stays_empty() {
    let (_, signature) = build_popup_input_region(320, 180, true, true);
    assert!(signature.reactive_rects.is_empty());
}

#[test]
fn hidden_stack_region_stays_empty() {
    let (_, signature) = build_popup_input_region(320, 180, false, false);
    assert!(signature.reactive_rects.is_empty());
}

#[test]
fn interactive_surface_retries_until_real_bounds_exist() {
    assert!(popup_surface_needs_retry(0, 180, false, true));
    assert!(popup_surface_needs_retry(320, 0, false, true));
    assert!(!popup_surface_needs_retry(320, 180, false, true));
    assert!(!popup_surface_needs_retry(0, 180, true, true));
    assert!(!popup_surface_needs_retry(0, 180, false, false));
}

#[test]
fn reset_runtime_state_clears_cached_signature() {
    let state = PopupInputRegionState::new(false);
    *state.last_signature.borrow_mut() = Some(InputRegionSignature {
        surface_width: 320,
        surface_height: 180,
        reactive_rects: vec![cairo::RectangleInt::new(0, 0, 320, 180)],
    });

    state.reset_runtime_state();

    assert!(state.last_signature.borrow().is_none());
    assert!(state.dirty.get());
    assert!(!state.retry_armed.get());
}
