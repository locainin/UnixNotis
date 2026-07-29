//! Popup window construction, placement, and input shaping

mod anchor;
mod build;
mod input_region;
mod monitor;
mod width_constraint;

pub(super) use build::{apply_popup_config, build_popup_window};
pub(super) use input_region::{refresh_popup_input_region, PopupInputRegionState};
