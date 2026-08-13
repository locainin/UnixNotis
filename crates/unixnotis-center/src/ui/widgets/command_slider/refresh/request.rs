//! Slider refresh request snapshots

use unixnotis_core::{CommandSpec, NumericParseMode, SliderWidgetConfig};

#[derive(Clone)]
pub(in super::super) struct SliderRefreshRequest {
    // Command used to read the current slider value
    pub(super) cmd: CommandSpec,
    // Lower bound used for parser clamping
    pub(super) min: f64,
    // Upper bound used for parser clamping
    pub(super) max: f64,
    // Step controls both parser tolerance and command formatting
    pub(super) step: f64,
    // Parser mode keeps backend-specific output handling explicit
    pub(super) parse_mode: NumericParseMode,
}

impl SliderRefreshRequest {
    pub(in super::super) fn from_config(config: &SliderWidgetConfig) -> Self {
        // Snapshot config values once so async callbacks do not borrow widget config
        Self {
            cmd: config.get_cmd.clone(),
            min: config.min,
            max: config.max,
            step: config.step,
            parse_mode: config.parse_mode,
        }
    }

    pub(in super::super) fn command(&self) -> String {
        // Action diagnostics need the command identity without exposing mutable request fields
        self.cmd.display_lossy()
    }
}

#[cfg(test)]
#[path = "tests/request.rs"]
mod tests;
