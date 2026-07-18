use std::cell::{Cell, RefCell};
use std::io;
use std::os::unix::process::ExitStatusExt;
use std::process::{ExitStatus, Output};
use std::rc::Rc;
use std::time::{Duration, Instant};

use gtk::prelude::*;
use unixnotis_core::NumericParseMode;

use super::{
    finish_refresh, handle_worker_result, next_refresh_generation, request_refresh,
    SliderRefreshRequest, SliderRefreshState,
};
use crate::ui::widgets::utils::command_slider::refresh::SliderRefreshGate;
use crate::ui::widgets::utils::RefreshBackoff;

#[test]
fn refresh_generation_increments_and_records_the_next_value() {
    let generation = Rc::new(Cell::new(41));

    assert_eq!(next_refresh_generation(&generation), 42);
    assert_eq!(generation.get(), 42);
}

#[test]
fn refresh_generation_wraps_without_panicking() {
    let generation = Rc::new(Cell::new(u64::MAX));

    assert_eq!(next_refresh_generation(&generation), 0);
    assert_eq!(generation.get(), 0);
}

#[gtk::test]
fn successful_worker_output_updates_slider_state() {
    let refresh = refresh_state();
    let request = request("read-slider");

    handle_worker_result(
        &request,
        &refresh,
        Ok(output(0, b"64")),
        Duration::from_secs(1),
    );

    assert_close(refresh.scale.value(), 64.0);
    assert_eq!(refresh.label.text(), "64%");
}

#[gtk::test]
fn unsuccessful_worker_status_does_not_apply_stdout() {
    let refresh = refresh_state();
    let request = request("read-slider");

    handle_worker_result(
        &request,
        &refresh,
        Ok(output(1, b"91")),
        Duration::from_secs(5),
    );

    assert_close(refresh.scale.value(), 0.0);
    assert_eq!(refresh.label.text(), "0%");
    assert!(!refresh
        .backoff
        .borrow()
        .should_refresh(Instant::now(), false));
}

#[gtk::test]
fn worker_transport_error_records_a_retry_deadline() {
    let refresh = refresh_state();
    let request = request("read-slider");

    handle_worker_result(
        &request,
        &refresh,
        Err(io::Error::other("worker channel failed")),
        Duration::from_secs(5),
    );

    assert_close(refresh.scale.value(), 0.0);
    assert!(!refresh
        .backoff
        .borrow()
        .should_refresh(Instant::now(), false));
}

#[gtk::test]
fn refresh_request_starts_worker_and_applies_current_generation() {
    let refresh = refresh_state();

    request_refresh(
        request("printf 42"),
        refresh.clone(),
        Duration::from_secs(1),
        true,
    );

    assert_eq!(refresh.refresh_gen.get(), 1);
    assert!(refresh.gate.is_in_flight());
    drain_refresh(&refresh);
    assert_close(refresh.scale.value(), 42.0);
    assert_eq!(refresh.label.text(), "42%");
}

#[gtk::test]
fn stale_worker_result_is_ignored_after_generation_changes() {
    let refresh = refresh_state();

    request_refresh(
        request("sleep 0.05; printf 91"),
        refresh.clone(),
        Duration::from_secs(1),
        true,
    );
    refresh.refresh_gen.set(2);

    drain_refresh(&refresh);
    assert_close(refresh.scale.value(), 0.0);
    assert_eq!(refresh.label.text(), "0%");
}

#[gtk::test]
fn refresh_respects_backoff_unless_forced() {
    let refresh = refresh_state();
    refresh
        .backoff
        .borrow_mut()
        .note_success(Instant::now(), Duration::from_mins(1), true);

    request_refresh(
        request("printf 55"),
        refresh.clone(),
        Duration::from_mins(1),
        false,
    );
    assert_eq!(refresh.refresh_gen.get(), 0);
    assert!(!refresh.gate.is_in_flight());

    request_refresh(
        request("printf 55"),
        refresh.clone(),
        Duration::from_mins(1),
        true,
    );
    assert_eq!(refresh.refresh_gen.get(), 1);
    drain_refresh(&refresh);
}

#[gtk::test]
fn refresh_gate_runs_one_queued_follow_up() {
    let refresh = refresh_state();
    assert!(refresh.gate.begin_or_queue());
    assert!(!refresh.gate.begin_or_queue());

    finish_refresh(
        request("printf 33"),
        refresh.clone(),
        Duration::from_secs(1),
    );

    assert_eq!(refresh.refresh_gen.get(), 1);
    assert!(refresh.gate.is_in_flight());
    drain_refresh(&refresh);
    assert_close(refresh.scale.value(), 33.0);
}

fn request(cmd: &str) -> SliderRefreshRequest {
    SliderRefreshRequest {
        cmd: cmd.to_string(),
        min: 0.0,
        max: 100.0,
        step: 1.0,
        parse_mode: NumericParseMode::Percent,
    }
}

fn refresh_state() -> SliderRefreshState {
    SliderRefreshState {
        scale: gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 100.0, 1.0),
        label: gtk::Label::new(Some("0%")),
        icon_image: gtk::Image::from_icon_name("test-normal"),
        updating: Rc::new(Cell::new(false)),
        refresh_gen: Rc::new(Cell::new(0)),
        icon_name: "test-normal".to_string(),
        icon_muted: Some("test-muted".to_string()),
        gate: SliderRefreshGate::new(),
        backoff: Rc::new(RefCell::new(RefreshBackoff::default())),
    }
}

fn output(code: i32, stdout: &[u8]) -> Output {
    Output {
        status: ExitStatus::from_raw(code << 8),
        stdout: stdout.to_vec(),
        stderr: Vec::new(),
    }
}

fn drain_refresh(refresh: &SliderRefreshState) {
    let context = glib::MainContext::default();
    let deadline = Instant::now() + Duration::from_secs(3);
    while refresh.gate.is_in_flight() && Instant::now() < deadline {
        while context.iteration(false) {}
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(
        !refresh.gate.is_in_flight(),
        "refresh should finish promptly"
    );
}

fn assert_close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < f64::EPSILON);
}
