use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant};

use gtk::prelude::*;
use unixnotis_core::{CommandSpec, SliderWidgetConfig};

use super::{attach_icon_action, attach_scale_action};
use crate::ui::widgets::utils::command_slider::refresh::{SliderRefreshGate, SliderRefreshMeta};
use crate::ui::widgets::utils::command_slider::view::build_slider_widgets;
use crate::ui::widgets::utils::RefreshBackoff;

#[gtk::test]
fn icon_action_adds_a_static_shell_when_toggle_command_is_absent() {
    let config = SliderWidgetConfig {
        toggle_cmd: None,
        ..SliderWidgetConfig::default()
    };
    let widgets = build_slider_widgets(&config, "test-slider");
    let refresh_meta = refresh_meta(&widgets.icon_name, widgets.icon_muted.as_deref());

    attach_icon_action(
        &widgets.root,
        &widgets.icon_image,
        &widgets.scale,
        &widgets.value_label,
        &config,
        &refresh_meta,
    );

    let icon_shell = widgets
        .root
        .first_child()
        .expect("icon shell should be prepended")
        .downcast::<gtk::Button>()
        .expect("icon shell should keep a stable button node");
    assert!(!icon_shell.can_target());
    assert!(!icon_shell.is_focusable());
}

#[gtk::test]
fn scale_action_echoes_the_changed_value_immediately() {
    let config = SliderWidgetConfig {
        set_cmd: CommandSpec::direct("true", [] as [&str; 0]),
        toggle_cmd: None,
        ..SliderWidgetConfig::default()
    };
    let widgets = build_slider_widgets(&config, "test-slider");
    let refresh_meta = refresh_meta(&widgets.icon_name, widgets.icon_muted.as_deref());
    attach_scale_action(
        &widgets.scale,
        &widgets.value_label,
        &widgets.icon_image,
        &config,
        &refresh_meta,
    );

    widgets.scale.set_value(37.0);

    assert_eq!(widgets.value_label.text(), "37%");
}

#[gtk::test]
fn successful_scale_action_keeps_the_local_value_without_corrective_refresh() {
    let config = SliderWidgetConfig {
        get_cmd: CommandSpec::direct("printf", ["22"]),
        set_cmd: CommandSpec::direct("true", [] as [&str; 0]),
        toggle_cmd: None,
        ..SliderWidgetConfig::default()
    };
    let widgets = build_slider_widgets(&config, "test-slider");
    let refresh_meta = refresh_meta(&widgets.icon_name, widgets.icon_muted.as_deref());
    attach_scale_action(
        &widgets.scale,
        &widgets.value_label,
        &widgets.icon_image,
        &config,
        &refresh_meta,
    );

    widgets.scale.set_value(37.0);
    iterate_main_context_for(Duration::from_millis(400));

    assert_eq!(refresh_meta.refresh_gen.get(), 0);
    assert_eq!(widgets.value_label.text(), "37%");
}

#[gtk::test]
fn failed_scale_action_runs_corrective_refresh() {
    let config = SliderWidgetConfig {
        get_cmd: CommandSpec::direct("printf", ["22"]),
        set_cmd: CommandSpec::direct("false", [] as [&str; 0]),
        toggle_cmd: None,
        ..SliderWidgetConfig::default()
    };
    let widgets = build_slider_widgets(&config, "test-slider");
    let refresh_meta = refresh_meta(&widgets.icon_name, widgets.icon_muted.as_deref());
    attach_scale_action(
        &widgets.scale,
        &widgets.value_label,
        &widgets.icon_image,
        &config,
        &refresh_meta,
    );

    widgets.scale.set_value(37.0);
    let deadline = Instant::now() + Duration::from_secs(3);
    while (refresh_meta.refresh_gen.get() == 0 || refresh_meta.gate.is_in_flight())
        && Instant::now() < deadline
    {
        iterate_main_context_for(Duration::from_millis(1));
    }

    assert_eq!(refresh_meta.refresh_gen.get(), 1);
    assert!(!refresh_meta.gate.is_in_flight());
    assert_eq!(widgets.value_label.text(), "22%");
}

fn refresh_meta(icon_name: &str, icon_muted: Option<&str>) -> SliderRefreshMeta {
    SliderRefreshMeta {
        updating: Rc::new(Cell::new(false)),
        refresh_gen: Rc::new(Cell::new(0)),
        icon_name: icon_name.to_string(),
        icon_muted: icon_muted.map(str::to_string),
        gate: SliderRefreshGate::new(),
        backoff: Rc::new(RefCell::new(RefreshBackoff::default())),
    }
}

fn iterate_main_context_for(duration: Duration) {
    let context = glib::MainContext::default();
    let deadline = Instant::now() + duration;
    while Instant::now() < deadline {
        while context.iteration(false) {}
        std::thread::sleep(Duration::from_millis(1));
    }
}
