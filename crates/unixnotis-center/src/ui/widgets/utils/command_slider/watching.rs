//! Command slider watch lifecycle

use std::time::Duration;

use super::super::{start_command_watch, CommandWatch};
use super::refresh::request_refresh;
use super::request::SliderRefreshRequest;
use super::CommandSlider;

pub(super) fn set_watch_active(slider: &CommandSlider, active: bool) {
    // Widgets without a watch command rely on polling only
    if slider.config.watch_cmd.is_none() {
        return;
    }

    let mut handle = slider.watch_handle.borrow_mut();
    if active {
        if handle.is_none() {
            *handle = start_watch(slider);
        }
    } else {
        handle.take();
    }
}

fn start_watch(slider: &CommandSlider) -> Option<CommandWatch> {
    // Watch callbacks reuse polling refresh logic to keep semantics consistent
    let cmd = slider.config.watch_cmd.as_ref()?;
    let request = SliderRefreshRequest::from_config(&slider.config);
    let refresh_state = slider.refresh_state();
    start_command_watch(cmd, move || {
        request_refresh(
            request.clone(),
            refresh_state.clone(),
            Duration::from_secs(1),
            true,
        );
    })
}
