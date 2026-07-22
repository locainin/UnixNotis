//! Command slider watch lifecycle

use std::time::Duration;

use super::super::CommandSlider;
use super::{request_refresh, SliderRefreshRequest};
use crate::ui::widgets::command_runtime::watch::{start_command_watch, CommandWatch};

pub(in super::super) fn set_watch_active(slider: &CommandSlider, active: bool) {
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

#[cfg(test)]
#[path = "tests/watch.rs"]
mod tests;
