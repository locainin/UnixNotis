//! Volume slider widget wrapper.

use unixnotis_core::SliderWidgetConfig;

use super::CommandSlider;

pub struct VolumeWidget {
    slider: CommandSlider,
}

impl VolumeWidget {
    pub fn new(config: SliderWidgetConfig) -> Self {
        Self {
            slider: CommandSlider::new(config, "unixnotis-quick-slider-volume"),
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
