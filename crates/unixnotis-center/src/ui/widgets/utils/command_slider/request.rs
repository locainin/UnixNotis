//! Slider refresh request data

use unixnotis_core::{NumericParseMode, SliderWidgetConfig};

#[derive(Clone)]
pub(super) struct SliderRefreshRequest {
    // Command used to read the current slider value
    pub(super) cmd: String,
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
    pub(super) fn from_config(config: &SliderWidgetConfig) -> Self {
        // Snapshot config values once so async callbacks do not borrow widget config
        Self {
            cmd: config.get_cmd.clone(),
            min: config.min,
            max: config.max,
            step: config.step,
            parse_mode: config.parse_mode,
        }
    }
}
