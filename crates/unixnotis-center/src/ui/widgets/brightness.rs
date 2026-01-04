//! Brightness slider widget wrapper.

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

    pub fn refresh(&self) {
        self.slider.refresh();
    }

    pub fn needs_polling(&self) -> bool {
        self.slider.needs_polling()
    }

    pub fn set_watch_active(&self, active: bool) {
        self.slider.set_watch_active(active);
    }
}
