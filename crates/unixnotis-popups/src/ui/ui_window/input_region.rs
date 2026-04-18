//! Popup input-region shaping for the popup surface
//!
//! This module keeps pointer hit-region behavior independent from window layout wiring

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gdk4_wayland::prelude::WaylandSurfaceExt;
use gtk::cairo;
use gtk::glib::object::Cast;
use gtk::prelude::*;

#[derive(Clone)]
pub(in super::super) struct PopupInputRegionState {
    // Runtime toggle for display-only passthrough mode
    allow_click_through: Rc<Cell<bool>>,
    // Dirty bit avoids rebuilding the region when nothing changed
    dirty: Rc<Cell<bool>>,
    // First-map retries stay bounded to one tick callback at a time
    retry_armed: Rc<Cell<bool>>,
    // Last applied signature skips no-op compositor region updates
    last_signature: Rc<RefCell<Option<InputRegionSignature>>>,
}

#[derive(Clone, PartialEq, Eq)]
struct InputRegionSignature {
    // Surface dimensions are part of identity so scale/output shifts are detected
    surface_width: i32,
    surface_height: i32,
    // Ordered rectangles keep signature comparisons simple and deterministic
    reactive_rects: Vec<cairo::RectangleInt>,
}

impl PopupInputRegionState {
    pub(super) fn new(allow_click_through: bool) -> Self {
        // New state starts dirty so first map applies a region immediately
        Self {
            allow_click_through: Rc::new(Cell::new(allow_click_through)),
            dirty: Rc::new(Cell::new(true)),
            retry_armed: Rc::new(Cell::new(false)),
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
        // Hidden windows should rebuild the region cleanly on the next map
        self.retry_armed.set(false);
        // Unmap can invalidate compositor-side surface state even when our last
        // in-process signature still looks current
        *self.last_signature.borrow_mut() = None;
        self.mark_dirty();
    }

    fn allow_click_through(&self) -> bool {
        self.allow_click_through.get()
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
) {
    // Any caller here observed a geometry or visibility change
    input_region.mark_dirty();
    let needs_retry = apply_popup_input_region_if_dirty(window, stack, input_region);
    if needs_retry {
        // The first refresh can land before GTK considers the window visible
        // Arm the retry anyway so the next mapped frame can finish the region update
        ensure_popup_input_region_retry(window, stack, input_region);
    }
}

fn ensure_popup_input_region_retry(
    window: &gtk::ApplicationWindow,
    stack: &gtk::Box,
    input_region: &PopupInputRegionState,
) {
    // One retry loop is enough to bridge the first map and first real allocation
    if input_region.retry_armed.replace(true) {
        return;
    }

    let stack = stack.clone();
    let input_region = input_region.clone();

    window.add_tick_callback(move |window, _| {
        let needs_retry = apply_popup_input_region_if_dirty(window, &stack, &input_region);
        if needs_retry {
            // Keep retrying until the popup surface reports real interactive bounds
            gtk::glib::ControlFlow::Continue
        } else {
            input_region.retry_armed.set(false);
            gtk::glib::ControlFlow::Break
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
    if popup_surface_needs_retry(
        surface_width,
        surface_height,
        input_region.allow_click_through(),
        has_visible_widgets,
    ) {
        // First map can run before the layer-shell surface has a usable size
        // Keep the region dirty until the compositor reports real bounds
        input_region.mark_dirty();
        return true;
    }

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
    force_wayland_commit_if_available(&surface);
    // Wayland surface state is pending until the next commit
    // Queue a redraw so the compositor sees the updated input region promptly
    surface.queue_render();
    *input_region.last_signature.borrow_mut() = Some(signature);
    false
}

fn force_wayland_commit_if_available(surface: &gtk::gdk::Surface) {
    // Wayland keeps surface state double-buffered until commit
    // For the first popup that can leave the new input region pending until some later change
    if let Ok(surface) = surface.clone().downcast::<gdk4_wayland::WaylandSurface>() {
        surface.force_next_commit();
    }
}

fn popup_surface_needs_retry(
    surface_width: i32,
    surface_height: i32,
    allow_click_through: bool,
    has_visible_widgets: bool,
) -> bool {
    // Interactive popups need a real mapped surface before they can own pointer input
    !allow_click_through && has_visible_widgets && (surface_width <= 0 || surface_height <= 0)
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
        if widget.get_visible() {
            return true;
        }
        child = next;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::{
        build_popup_input_region, popup_surface_needs_retry, InputRegionSignature,
        PopupInputRegionState,
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
}
