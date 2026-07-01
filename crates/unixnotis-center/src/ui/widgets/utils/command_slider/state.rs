//! Slider refresh widget state

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::super::RefreshBackoff;
use super::gate::SliderRefreshGate;

#[derive(Clone)]
pub(super) struct SliderRefreshState {
    // Slider updated from command output
    pub(super) scale: gtk::Scale,
    // Label kept in sync with the slider
    pub(super) label: gtk::Label,
    // Icon image updated after refresh
    pub(super) icon_image: gtk::Image,
    // Guard stops refresh writes from triggering another set command
    pub(super) updating: Rc<Cell<bool>>,
    // Generation drops stale async refresh results
    pub(super) refresh_gen: Rc<Cell<u64>>,
    // Normal icon shown when not muted
    pub(super) icon_name: String,
    // Optional icon used when muted
    pub(super) icon_muted: Option<String>,
    // Local gate keeps refresh bursts bounded to one running and one pending
    pub(super) gate: SliderRefreshGate,
    // Polling backoff is shared with the owning slider
    pub(super) backoff: Rc<RefCell<RefreshBackoff>>,
}

#[derive(Clone)]
pub(super) struct SliderRefreshMeta {
    // Non-widget refresh state that is safe to hold across signal closures
    pub(super) updating: Rc<Cell<bool>>,
    // Generation drops stale async refresh results
    pub(super) refresh_gen: Rc<Cell<u64>>,
    // Normal icon shown when not muted
    pub(super) icon_name: String,
    // Optional icon used when muted
    pub(super) icon_muted: Option<String>,
    // Local gate keeps refresh bursts bounded to one running and one pending
    pub(super) gate: SliderRefreshGate,
    // Polling backoff is shared with short-lived refresh state
    pub(super) backoff: Rc<RefCell<RefreshBackoff>>,
}

pub(super) fn build_refresh_state_from_weak(
    scale: &glib::WeakRef<gtk::Scale>,
    label: &glib::WeakRef<gtk::Label>,
    icon_image: &glib::WeakRef<gtk::Image>,
    refresh_meta: &SliderRefreshMeta,
) -> Option<SliderRefreshState> {
    // Widget teardown is normal, so stale async completions just stop here
    let scale = scale.upgrade()?;
    let label = label.upgrade()?;
    let icon_image = icon_image.upgrade()?;
    Some(SliderRefreshState {
        scale,
        label,
        icon_image,
        updating: refresh_meta.updating.clone(),
        refresh_gen: refresh_meta.refresh_gen.clone(),
        icon_name: refresh_meta.icon_name.clone(),
        icon_muted: refresh_meta.icon_muted.clone(),
        gate: refresh_meta.gate.clone(),
        backoff: refresh_meta.backoff.clone(),
    })
}
