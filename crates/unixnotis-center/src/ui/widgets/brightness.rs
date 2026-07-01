//! Brightness slider widget wrapper.

use std::time::{Duration, Instant};

use unixnotis_core::SliderWidgetConfig;

use super::CommandSlider;

pub struct BrightnessWidget {
    slider: CommandSlider,
}

impl BrightnessWidget {
    pub fn new(config: SliderWidgetConfig) -> Self {
        let mut config = config;
        // Brightness control does not support toggle actions.
        config.toggle_cmd = None;
        config.icon_muted = None;
        Self {
            slider: CommandSlider::new(config, "unixnotis-quick-slider-brightness"),
        }
    }

    pub fn root(&self) -> &gtk::Box {
        &self.slider.root
    }

    pub fn refresh(&self, base_interval: Duration, force: bool) {
        self.slider.refresh(base_interval, force);
    }

    pub fn next_poll_in(&self, now: Instant, base_interval: Duration) -> Option<Duration> {
        self.slider.next_poll_in(now, base_interval)
    }

    pub fn set_watch_active(&self, active: bool) {
        self.slider.set_watch_active(active);
    }
}
