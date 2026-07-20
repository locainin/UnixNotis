use unixnotis_core::{CommandSpec, SliderWidgetConfig};

use super::set_watch_active;
use crate::ui::widgets::utils::command_slider::CommandSlider;

#[gtk::test]
fn watch_lifecycle_starts_once_and_stops_cleanly() {
    let config = SliderWidgetConfig {
        watch_cmd: Some(CommandSpec::direct("sleep", ["2"])),
        ..SliderWidgetConfig::default()
    };
    let slider = CommandSlider::new(config, "test-slider");

    set_watch_active(&slider, true);
    assert!(slider.watch_handle.borrow().is_some());

    // Repeated activation must retain one live watcher
    set_watch_active(&slider, true);
    assert!(slider.watch_handle.borrow().is_some());

    set_watch_active(&slider, false);
    assert!(slider.watch_handle.borrow().is_none());
}

#[gtk::test]
fn slider_without_watch_command_ignores_lifecycle_changes() {
    let slider = CommandSlider::new(SliderWidgetConfig::default(), "test-slider");

    set_watch_active(&slider, true);
    set_watch_active(&slider, false);

    assert!(slider.watch_handle.borrow().is_none());
}
