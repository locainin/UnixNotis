//! Popup input-region shaping and animation tracking
//!
//! This module keeps pointer hit-region behavior independent from window layout wiring

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::cairo;
use gtk::glib::ControlFlow;
use gtk::prelude::*;

#[derive(Clone)]
pub(in super::super) struct PopupInputRegionState {
    // Runtime toggle for display-only passthrough mode
    allow_click_through: Rc<Cell<bool>>,
    // Dirty bit avoids rebuilding the region when nothing changed
    dirty: Rc<Cell<bool>>,
    // Guard prevents duplicate tick callbacks
    ticking: Rc<Cell<bool>>,
    // Last applied signature skips no-op compositor region updates
    last_signature: Rc<RefCell<Option<InputRegionSignature>>>,
}

#[derive(Clone, PartialEq, Eq)]
struct InputRegionSignature {
    // Surface dimensions are part of identity so scale/output shifts are detected
    surface_width: i32,
    surface_height: i32,
    // Ordered source rectangles represent clickable popup regions
    reactive_rects: Vec<cairo::RectangleInt>,
}

impl PopupInputRegionState {
    pub(super) fn new(allow_click_through: bool) -> Self {
        // New state starts dirty so first map applies a region immediately
        Self {
            allow_click_through: Rc::new(Cell::new(allow_click_through)),
            dirty: Rc::new(Cell::new(true)),
            ticking: Rc::new(Cell::new(false)),
            last_signature: Rc::new(RefCell::new(None)),
        }
    }

    pub(super) fn set_allow_click_through(&self, allow_click_through: bool) {
        // Only invalidate when mode actually changes
        if self.allow_click_through.replace(allow_click_through) != allow_click_through {
            self.mark_dirty();
        }
    }

    pub(super) fn reset_runtime_state(&self) {
        // Hidden windows should not keep the old tick guard alive
        self.ticking.set(false);
        self.mark_dirty();
    }

    fn allow_click_through(&self) -> bool {
        self.allow_click_through.get()
    }

    fn tracks_geometry(&self) -> bool {
        // Interactive popups now use the full mapped surface as the hit region
        // so animation ticks are no longer needed to chase child geometry
        false
    }

    fn mark_dirty(&self) {
        // Dirty marks request a full rebuild on next apply pass
        self.dirty.set(true);
    }

    fn take_dirty(&self) -> bool {
        self.dirty.replace(false)
    }
}

pub(in super::super) fn refresh_popup_input_region(
    window: &gtk::ApplicationWindow,
    stack: &gtk::Box,
    input_region: &PopupInputRegionState,
    keep_ticking: bool,
) {
    // Any caller here observed a geometry or visibility change
    input_region.mark_dirty();
    let needs_retry = apply_popup_input_region_if_dirty(window, stack, input_region);

    if !input_region.tracks_geometry() {
        // Click-through mode keeps an empty region, so animation ticks would be wasted work
        return;
    }

    // Keep ticking only during animations or when geometry is still settling
    if window.is_visible() && (keep_ticking || needs_retry) {
        // Tick callback self-terminates once transitions and retries are complete
        ensure_popup_input_region_tick(window, stack, input_region);
    }
}

pub(in super::super) fn popup_stack_has_active_transitions(stack: &gtk::Box) -> bool {
    let mut child = stack.first_child();
    while let Some(widget) = child {
        let next = widget.next_sibling();

        // Revealers animate between these two states
        if let Ok(revealer) = widget.clone().downcast::<gtk::Revealer>() {
            // Transition is active while target and current child reveal states differ
            if revealer.reveals_child() != revealer.is_child_revealed() {
                return true;
            }
        }

        child = next;
    }
    false
}

fn ensure_popup_input_region_tick(
    window: &gtk::ApplicationWindow,
    stack: &gtk::Box,
    input_region: &PopupInputRegionState,
) {
    // Avoid duplicate callback loops when many updates arrive in one frame
    if input_region.ticking.replace(true) {
        return;
    }

    let stack = stack.clone();
    let input_region = input_region.clone();

    window.add_tick_callback(move |window, _| {
        // Animation-aware refresh keeps hitboxes aligned with revealer motion
        let active_transitions = popup_stack_has_active_transitions(&stack);
        if active_transitions {
            // Animated revealers shift geometry frame-by-frame
            input_region.mark_dirty();
        }

        let needs_retry = apply_popup_input_region_if_dirty(window, &stack, &input_region);
        if active_transitions || needs_retry {
            ControlFlow::Continue
        } else {
            // Reset guard so future animations can re-arm the tick callback
            input_region.ticking.set(false);
            ControlFlow::Break
        }
    });
}

fn apply_popup_input_region_if_dirty(
    window: &gtk::ApplicationWindow,
    stack: &gtk::Box,
    input_region: &PopupInputRegionState,
) -> bool {
    // Skip work when no geometry or visibility changes have been observed
    if !input_region.take_dirty() {
        return false;
    }

    let Some(surface) = window.surface() else {
        // Surface can be unavailable very early in lifecycle
        input_region.mark_dirty();
        return true;
    };

    let surface_width = surface.width().max(0);
    let surface_height = surface.height().max(0);

    let has_visible_widgets = popup_stack_has_visible_widgets(stack);
    // Signature includes surface size so monitor/scale changes are detected
    let (region, signature) = build_popup_input_region(
        surface_width,
        surface_height,
        input_region.allow_click_through(),
        has_visible_widgets,
    );

    let unchanged = input_region
        .last_signature
        .borrow()
        .as_ref()
        .is_some_and(|prev| *prev == signature);
    if unchanged {
        // No compositor call is needed when geometry signature did not move
        return false;
    }

    // In interactive mode the whole popup surface is reactive
    // In click-through mode the region stays empty so the compositor ignores it
    surface.set_input_region(&region);
    *input_region.last_signature.borrow_mut() = Some(signature);
    false
}

fn build_popup_input_region(
    surface_width: i32,
    surface_height: i32,
    allow_click_through: bool,
    has_visible_widgets: bool,
) -> (cairo::Region, InputRegionSignature) {
    let region = cairo::Region::create();
    let reactive_rects =
        if allow_click_through || !has_visible_widgets || surface_width <= 0 || surface_height <= 0
        {
            // Click-through mode should never intercept pointer events
            // Hidden stacks also keep an empty region so stale hit boxes cannot survive
            Vec::new()
        } else {
            // Interactive popups use the whole mapped surface as their hit region
            // This avoids stale partial masks that can make action buttons unclickable
            let rect = cairo::RectangleInt::new(0, 0, surface_width, surface_height);
            let _ = region.union_rectangle(&rect);
            vec![rect]
        };

    (
        region,
        InputRegionSignature {
            surface_width,
            surface_height,
            reactive_rects,
        },
    )
}

fn popup_stack_has_visible_widgets(stack: &gtk::Box) -> bool {
    let mut child = stack.first_child();
    while let Some(widget) = child {
        let next = widget.next_sibling();
        if widget.is_visible() {
            return true;
        }
        child = next;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::{build_popup_input_region, popup_stack_has_visible_widgets, PopupInputRegionState};
    use gtk::cairo;
    use gtk::prelude::{BoxExt, WidgetExt};

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
    fn popup_input_state_tracks_geometry_is_disabled() {
        let state = PopupInputRegionState::new(false);
        assert!(!state.tracks_geometry());
    }

    #[test]
    fn popup_stack_visibility_helper_detects_visible_children() {
        gtk::init().ok();
        let stack = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let hidden = gtk::Label::new(Some("hidden"));
        hidden.set_visible(false);
        stack.append(&hidden);
        assert!(!popup_stack_has_visible_widgets(&stack));

        let visible = gtk::Label::new(Some("visible"));
        visible.set_visible(true);
        stack.append(&visible);
        assert!(popup_stack_has_visible_widgets(&stack));
    }
}
