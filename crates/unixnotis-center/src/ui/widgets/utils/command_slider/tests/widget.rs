use std::time::{Duration, Instant};

use gtk::prelude::*;
use unixnotis_core::{css::hooks, CommandSpec, SliderWidgetConfig};

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

    assert_eq!(
        slider.next_poll_in(Instant::now(), Duration::from_secs(5)),
        Some(Duration::from_millis(1)),
        "a slider without an active watch needs the initial polling deadline"
    );
}

#[gtk::test]
fn public_refresh_starts_and_completes_slider_update() {
    let config = SliderWidgetConfig {
        get_cmd: CommandSpec::direct("printf", ["37"]),
        ..SliderWidgetConfig::default()
    };
    let slider = CommandSlider::new(config, "volume-slider");

    slider.refresh(Duration::from_secs(1), true);

    assert_eq!(slider.refresh_gen.get(), 1);
    assert!(slider.refresh_gate.is_in_flight());
    let context = glib::MainContext::default();
    let deadline = Instant::now() + Duration::from_secs(3);
    while slider.refresh_gate.is_in_flight() && Instant::now() < deadline {
        while context.iteration(false) {}
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(!slider.refresh_gate.is_in_flight());
    assert_close(slider.scale.value(), 37.0);
    assert_eq!(slider.value_label.text(), "37%");
}

#[gtk::test]
fn public_refresh_honors_recorded_backoff() {
    let slider = CommandSlider::new(SliderWidgetConfig::default(), "volume-slider");
    slider
        .refresh_backoff
        .borrow_mut()
        .note_success(Instant::now(), Duration::from_mins(1), true);

    slider.refresh(Duration::from_mins(1), false);

    assert_eq!(slider.refresh_gen.get(), 0);
    assert!(!slider.refresh_gate.is_in_flight());
}

#[gtk::test]
fn public_watch_lifecycle_controls_the_owned_handle() {
    let config = SliderWidgetConfig {
        watch_cmd: Some(CommandSpec::direct("sleep", ["2"])),
        ..SliderWidgetConfig::default()
    };
    let slider = CommandSlider::new(config, "volume-slider");

    slider.set_watch_active(true);
    assert!(slider.watch_handle.borrow().is_some());

    slider.set_watch_active(false);
    assert!(slider.watch_handle.borrow().is_none());
}

fn assert_close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < f64::EPSILON);
}
