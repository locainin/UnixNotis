use gtk::prelude::*;
use unixnotis_core::SliderWidgetConfig;

use super::build_slider_widgets;

#[gtk::test]
fn slider_builder_applies_configured_range_and_value_visibility() {
    let config = SliderWidgetConfig {
        min: 10.0,
        max: 90.0,
        step: 5.0,
        show_value: false,
        ..SliderWidgetConfig::default()
    };

    let widgets = build_slider_widgets(&config, "test-slider");

    assert_close(widgets.scale.adjustment().lower(), 10.0);
    assert_close(widgets.scale.adjustment().upper(), 90.0);
    assert_close(widgets.scale.adjustment().step_increment(), 5.0);
    assert!(!widgets.value_label.is_visible());
    assert!(widgets.root.has_css_class("test-slider"));
}

fn assert_close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < f64::EPSILON);
}
