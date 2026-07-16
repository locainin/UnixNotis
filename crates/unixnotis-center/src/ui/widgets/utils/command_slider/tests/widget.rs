use std::time::{Duration, Instant};

use gtk::prelude::*;
use unixnotis_core::{css::hooks, SliderWidgetConfig};

use super::CommandSlider;

#[gtk::test]
fn slider_shell_keeps_base_and_variant_css_hooks() {
    let slider = CommandSlider::new(SliderWidgetConfig::default(), "volume-slider");

    assert!(
        slider.root.has_css_class(hooks::slider::ROOT),
        "slider root should retain the shared theme hook"
    );
    assert!(
        slider.root.has_css_class("volume-slider"),
        "slider root should retain its caller-provided variant"
    );
}

#[gtk::test]
fn inactive_watch_slider_remains_eligible_for_polling() {
    let slider = CommandSlider::new(SliderWidgetConfig::default(), "volume-slider");

    assert!(
        slider
            .next_poll_in(Instant::now(), Duration::from_secs(5))
            .is_some(),
        "a slider without an active watch needs a polling deadline"
    );
}
