use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant};

use gtk::prelude::*;
use unixnotis_core::{CommandSpec, NumericParseMode};

use super::{apply_slider_icon, apply_slider_value, apply_successful_output, note_slider_error};
use crate::ui::widgets::utils::command_slider::refresh::{
    SliderRefreshGate, SliderRefreshRequest, SliderRefreshState,
};
use crate::ui::widgets::utils::RefreshBackoff;

#[gtk::test]
fn slider_value_application_updates_only_changed_widget_state() {
    let request = request();
    let refresh = refresh_state(Some("audio-volume-muted-symbolic"));
    refresh.scale.set_value(25.0);
    refresh.label.set_text("25%");

    assert!(!apply_slider_value(&request, &refresh, 25.0));

    refresh.label.set_text("stale");
    assert!(apply_slider_value(&request, &refresh, 25.0));
    assert_eq!(refresh.label.text(), "25%");
    assert_close(refresh.scale.value(), 25.0);

    assert!(apply_slider_value(&request, &refresh, 40.0));
    assert_eq!(refresh.label.text(), "40%");
    assert_close(refresh.scale.value(), 40.0);
    assert!(!refresh.updating.get());
}

#[gtk::test]
fn slider_icon_application_handles_optional_and_unchanged_icons() {
    let refresh = refresh_state(Some("audio-volume-muted-symbolic"));

    assert!(!apply_slider_icon(&refresh, false));
    assert!(apply_slider_icon(&refresh, true));
    assert_eq!(
        refresh.icon_image.icon_name().as_deref(),
        Some("audio-volume-muted-symbolic")
    );
    assert!(!apply_slider_icon(&refresh, true));

    let refresh_without_muted_icon = refresh_state(None);
    assert!(!apply_slider_icon(&refresh_without_muted_icon, true));
}

#[gtk::test]
fn successful_output_resets_backoff_when_only_the_icon_changes() {
    let request = request();
    let refresh = refresh_state(Some("audio-volume-muted-symbolic"));
    let base_interval = Duration::from_secs(100);
    refresh.scale.set_value(25.0);
    refresh.label.set_text("25%");

    // Two stable reads establish a longer delay before the icon-only change
    let now = Instant::now();
    refresh
        .backoff
        .borrow_mut()
        .note_success(now, base_interval, false);
    refresh
        .backoff
        .borrow_mut()
        .note_success(now, base_interval, false);

    apply_successful_output(&request, &refresh, b"25 muted", base_interval);

    let remaining = refresh
        .backoff
        .borrow()
        .next_due_in(Instant::now())
        .expect("successful refresh should set a deadline");
    assert!(remaining <= base_interval);
    assert!(remaining > Duration::from_secs(99));
    assert_eq!(
        refresh.icon_image.icon_name().as_deref(),
        Some("audio-volume-muted-symbolic")
    );
}

#[gtk::test]
fn invalid_output_records_an_error_without_changing_widgets() {
    let request = request();
    let refresh = refresh_state(Some("audio-volume-muted-symbolic"));
    let base_interval = Duration::from_secs(10);
    refresh.scale.set_value(25.0);
    refresh.label.set_text("25%");

    apply_successful_output(&request, &refresh, b"not-a-number", base_interval);

    assert_close(refresh.scale.value(), 25.0);
    assert_eq!(refresh.label.text(), "25%");
    assert!(!refresh
        .backoff
        .borrow()
        .should_refresh(Instant::now(), false));
}

#[gtk::test]
fn slider_error_records_a_retry_deadline() {
    let refresh = refresh_state(None);

    note_slider_error(&refresh, Duration::from_secs(10));

    assert!(!refresh
        .backoff
        .borrow()
        .should_refresh(Instant::now(), false));
}

fn request() -> SliderRefreshRequest {
    SliderRefreshRequest {
        cmd: CommandSpec::direct("read-slider", [] as [&str; 0]),
        min: 0.0,
        max: 100.0,
        step: 1.0,
        parse_mode: NumericParseMode::Percent,
    }
}

fn refresh_state(icon_muted: Option<&str>) -> SliderRefreshState {
    let scale = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 100.0, 1.0);
    let label = gtk::Label::new(Some("0%"));
    let icon_image = gtk::Image::from_icon_name("audio-volume-high-symbolic");

    SliderRefreshState {
        scale,
        label,
        icon_image,
        updating: Rc::new(Cell::new(false)),
        refresh_gen: Rc::new(Cell::new(0)),
        icon_name: "audio-volume-high-symbolic".to_string(),
        icon_muted: icon_muted.map(str::to_string),
        gate: SliderRefreshGate::new(),
        backoff: Rc::new(RefCell::new(RefreshBackoff::default())),
    }
}

fn assert_close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < f64::EPSILON);
}
